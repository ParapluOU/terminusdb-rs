:- module('plugins/changeset_sse', []).

%% TerminusDB SSE Changeset Plugin with Document Change Detection
%% Based on changeset-webhook.pl approach

:- use_module(library(broadcast)).
:- use_module(library(http/http_dispatch)).
:- use_module(library(http/json)).
:- use_module(library(terminus_store)).
:- use_module(core(transaction)).
:- use_module(core(util)).
:- use_module(core(query)).
:- use_module(core(account)).

%% Register SSE route
:- catch(
    http_handler(api(changesets/stream),
                 routes:cors_handler(Method, 'plugins/changeset_sse':changeset_sse_handler),
                 [method(Method), methods([options,get])]),
    E,
    json_log_error_formatted("Failed to register SSE handler: ~q", [E])
).

%% Post-commit hook with document change detection
plugins:post_commit_hook(Validation_Objects, Meta_Data) :-
    [Validation_Object|_] = Validation_Objects,
    is_dict(Validation_Object.descriptor, branch_descriptor),
    !,

    % Extract commit metadata
    Branch_Name = (Validation_Object.descriptor.branch_name),
    Commit_Graph = (Validation_Object.parent),
    branch_head_commit(Commit_Graph, Branch_Name, Commit_Uri),
    commit_uri_to_metadata(Commit_Graph, Commit_Uri, Author, Message, Timestamp),
    commit_id_uri(Commit_Graph, Commit_Id, Commit_Uri),

    Deletes = (Meta_Data.deletes),
    Inserts = (Meta_Data.inserts),

    resolve_absolute_string_descriptor(Resource, (Validation_Object.descriptor)),

    % Detect document changes using terminus_store layer inspection
    (   Validation_Object.instance_objects = [Instance_Object|_],
        Layer = Instance_Object.read
    ->  catch(
            detect_document_changes(Layer, Changes, Added_Count, Deleted_Count, Updated_Count),
            Error,
            (json_log_error_formatted("SSE Plugin: Error detecting changes: ~q", [Error]),
             Changes = [], Added_Count = 0, Deleted_Count = 0, Updated_Count = 0)
        )
    ;   Changes = [], Added_Count = 0, Deleted_Count = 0, Updated_Count = 0
    ),

    % Build comprehensive payload
    Payload = json{
                  resource: Resource,
                  branch: Branch_Name,
                  commit: json{
                      id: Commit_Id,
                      author: Author,
                      message: Message,
                      timestamp: Timestamp
                  },
                  metadata: json{
                      inserts_count: Inserts,
                      deletes_count: Deletes,
                      documents_added: Added_Count,
                      documents_deleted: Deleted_Count,
                      documents_updated: Updated_Count
                  },
                  changes: Changes
              },

    broadcast(changeset(Payload)),
    json_log_info_formatted("SSE Plugin: Broadcast changeset ~q with ~w changes", [Commit_Id, length(Changes)]).

plugins:post_commit_hook(_Validation_Objects, _Meta_Data).

%% Check if user has read access to a database
has_changeset_access(System_DB, Auth, Resource) :-
    % Parse resource string: "org/db/repo/branch/name" -> extract org and db
    split_string(Resource, "/", "", [Org_String, DB_String | _]),
    atom_string(Organization_Name, Org_String),
    atom_string(Database_Name, DB_String),

    json_log_info_formatted("SSE Plugin: Checking access for ~q to ~q/~q", [Auth, Organization_Name, Database_Name]),

    % Create database descriptor
    Database_Descriptor = database_descriptor{
        organization_name: Organization_Name,
        database_name: Database_Name
    },

    % Check read access - will fail if no permission
    catch(
        (askable_context(Database_Descriptor, System_DB, Auth, Context),
         assert_read_access(Context),
         json_log_info_formatted("SSE Plugin: Access granted for ~q", [Auth])),
        Error,
        (json_log_error_formatted("SSE Plugin: Access check error: ~q", [Error]), fail)
    ).

%% Detect document changes from layer
detect_document_changes(Layer, Limited_Changes, Added_Count, Deleted_Count, Updated_Count) :-
    % Get rdf:type predicate ID
    global_prefix_expand(rdf:type, Rdf_Type),
    terminus_store:predicate_id(Layer, Rdf_Type, Rdf_Type_Id),

    % Find added documents (new rdf:type triples, no removals)
    findall(json{id: Subject_String, action: added},
            (terminus_store:id_triple_addition(Layer, S_Id, Rdf_Type_Id, _),
             terminus_store:subject_id(Layer, Subject_String, S_Id),
             \+ terminus_store:id_triple_removal(Layer, S_Id, _, _)),
            Added_Docs),

    % Find deleted documents (removed rdf:type triples, no additions)
    findall(json{id: Subject_String, action: deleted},
            (terminus_store:id_triple_removal(Layer, S_Id, Rdf_Type_Id, _),
             terminus_store:subject_id(Layer, Subject_String, S_Id),
             \+ terminus_store:id_triple_addition(Layer, S_Id, _, _)),
            Deleted_Docs),

    % Find updated documents (both additions and removals for same subject)
    findall(json{id: Subject_String, action: updated},
            distinct(Subject_String,
                    (terminus_store:id_triple_addition(Layer, S_Id, _, _),
                     terminus_store:id_triple_removal(Layer, S_Id, _, _),
                     terminus_store:subject_id(Layer, Subject_String, S_Id))),
            Updated_Docs),

    % Get counts
    length(Added_Docs, Added_Count),
    length(Deleted_Docs, Deleted_Count),
    length(Updated_Docs, Updated_Count),

    % Combine and limit to 50
    append([Added_Docs, Deleted_Docs, Updated_Docs], All_Changes),
    list_to_set(All_Changes, Unique_Changes),
    length(Unique_Changes, Changes_Count),
    (   Changes_Count > 50
    ->  append(Limited_Changes, _, Unique_Changes),
        length(Limited_Changes, 50)
    ;   Limited_Changes = Unique_Changes
    ),

    json_log_info_formatted("SSE Plugin: Detected ~w changes (~w added, ~w deleted, ~w updated)",
                          [Changes_Count, Added_Count, Deleted_Count, Updated_Count]).

%% SSE Handler
changeset_sse_handler(get, _Request, System_DB, Auth) :-
    json_log_info_formatted("SSE Plugin: Client connecting", []),

    format('Status: 200~n'),
    format('Content-Type: text/event-stream~n'),
    format('Cache-Control: no-cache~n'),
    format('Connection: keep-alive~n'),
    format('Transfer-Encoding: chunked~n~n'),
    flush_output,

    format(': connected~n~n'),
    flush_output,

    current_output(Out),

    gensym(sse_listener_, Listener_Id),
    % Pass System_DB and Auth to the callback for permission checking
    listen(Listener_Id, changeset(Payload), sse_send_event_if_authorized(Out, System_DB, Auth, Payload)),

    json_log_info_formatted("SSE Plugin: Client ~q connected", [Listener_Id]),

    catch(
        sse_keep_alive_loop(Listener_Id),
        E,
        json_log_info_formatted("SSE Plugin: Client ~q disconnected: ~q", [Listener_Id, E])
    ),

    unlisten(Listener_Id),
    json_log_info_formatted("SSE Plugin: Client ~q cleaned up", [Listener_Id]).

%% Send SSE event only if user has access to the database
sse_send_event_if_authorized(Out, System_DB, Auth, Payload) :-
    Resource = (Payload.resource),
    (   has_changeset_access(System_DB, Auth, Resource)
    ->  % User has access - send event
        catch(
            (
                format(Out, 'event: changeset~n', []),
                atom_json_dict(Json_Atom, Payload, [width(0)]),
                format(Out, 'data: ~w~n~n', [Json_Atom]),
                flush_output(Out),
                json_log_info_formatted("SSE Plugin: Sent changeset event for ~q", [Resource])
            ),
            E,
            json_log_error_formatted("SSE Plugin: Send error ~q", [E])
        )
    ;   % User doesn't have access - skip silently
        json_log_info_formatted("SSE Plugin: Skipped changeset for ~q (no access)", [Resource])
    ).

%% Keep-alive loop
sse_keep_alive_loop(Listener_Id) :-
    get_time(Start),
    sse_loop_inner(Listener_Id, Start).

sse_loop_inner(Listener_Id, Last_Heartbeat) :-
    sleep(10),
    get_time(Now),
    (   Now - Last_Heartbeat > 30
    ->  format(': heartbeat~n~n'),
        flush_output,
        New_Heartbeat = Now
    ;   New_Heartbeat = Last_Heartbeat
    ),
    sse_loop_inner(Listener_Id, New_Heartbeat).
