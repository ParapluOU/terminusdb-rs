module Api exposing
    ( fetchNodes
    , fetchStatuses
    , createNode
    , updateNode
    , deleteNode
    , getDatabases
    , getDatabaseSchema
    , getDatabaseCommits
    , getDatabaseRemotes
    , addRemote
    , deleteRemote
    , nodeDecoder
    , statusDecoder
    )

{-| HTTP API client for TerminusDB Manager backend
-}

import Http
import Json.Decode as Decode exposing (Decoder, field, string, int, bool, float, list, maybe)
import Json.Encode as Encode
import Types exposing (Node, NodeStatus, RemoteInfo, NodeForm, Connectivity(..), DatabaseInfo, ModelInfo, CommitInfo)
import Url


-- DECODERS


nodeDecoder : Decoder Node
nodeDecoder =
    Decode.map8
        (\id label host portNumber username password sshEnabled positionX ->
            \positionY ->
                { id = id
                , label = label
                , host = host
                , portNumber = portNumber
                , username = username
                , password = password
                , sshEnabled = sshEnabled
                , positionX = positionX
                , positionY = positionY
                }
        )
        (field "id" string)
        (field "label" string)
        (field "host" string)
        (field "port" int)  -- JSON uses "port", Elm uses "portNumber"
        (field "username" string)
        (field "password" string)
        (field "ssh_enabled" bool)
        (field "position_x" float)
        |> Decode.andThen (\fn -> Decode.map fn (field "position_y" float))


remoteInfoDecoder : Decoder RemoteInfo
remoteInfoDecoder =
    Decode.map4 RemoteInfo
        (field "database" string)
        (field "remote_name" string)
        (field "remote_url" string)
        (field "target_node_id" (maybe string))


connectivityDecoder : Decoder Connectivity
connectivityDecoder =
    string
        |> Decode.andThen
            (\str ->
                case str of
                    "unreachable" ->
                        Decode.succeed Unreachable

                    "reachable" ->
                        Decode.succeed Reachable

                    "accessible" ->
                        Decode.succeed Accessible

                    _ ->
                        Decode.fail ("Unknown connectivity: " ++ str)
            )


statusDecoder : Decoder NodeStatus
statusDecoder =
    Decode.map7 NodeStatus
        (field "node_id" string)
        (field "online" bool)
        (field "connectivity" connectivityDecoder)
        (field "database_count" int)
        (field "remotes" (list remoteInfoDecoder))
        (field "last_check" string)
        (maybe (field "error" string))


databaseInfoDecoder : Decoder DatabaseInfo
databaseInfoDecoder =
    Decode.map4 DatabaseInfo
        (field "name" string)
        (field "commitCount" int)
        (field "lastModified" string)
        (field "remoteCount" int)


modelInfoDecoder : Decoder ModelInfo
modelInfoDecoder =
    Decode.map2 ModelInfo
        (field "name" string)
        (field "instanceCount" int)


commitInfoDecoder : Decoder CommitInfo
commitInfoDecoder =
    Decode.map4 CommitInfo
        (field "id" string)
        (field "author" string)
        (field "message" string)
        (field "timestamp" string)


-- ENCODERS


encodeNodeForm : NodeForm -> Encode.Value
encodeNodeForm form =
    Encode.object
        [ ( "label", Encode.string form.label )
        , ( "host", Encode.string form.host )
        , ( "port", Encode.int (Maybe.withDefault 6363 (String.toInt form.portNumber)) )
        , ( "username", Encode.string form.username )
        , ( "password", Encode.string form.password )
        , ( "ssh_enabled", Encode.bool form.sshEnabled )
        , ( "position_x", Encode.float form.positionX )
        , ( "position_y", Encode.float form.positionY )
        ]


encodeNodeUpdate : Node -> Encode.Value
encodeNodeUpdate node =
    Encode.object
        [ ( "label", Encode.string node.label )
        , ( "host", Encode.string node.host )
        , ( "port", Encode.int node.portNumber )
        , ( "username", Encode.string node.username )
        , ( "password", Encode.string node.password )
        , ( "ssh_enabled", Encode.bool node.sshEnabled )
        , ( "position_x", Encode.float node.positionX )
        , ( "position_y", Encode.float node.positionY )
        ]


encodePositionUpdate : Float -> Float -> Encode.Value
encodePositionUpdate x y =
    Encode.object
        [ ( "position_x", Encode.float x )
        , ( "position_y", Encode.float y )
        ]


encodeAddRemote : String -> String -> Encode.Value
encodeAddRemote remoteName remoteUrl =
    Encode.object
        [ ( "remoteName", Encode.string remoteName )
        , ( "remoteUrl", Encode.string remoteUrl )
        ]


-- API CALLS


fetchNodes : (Result Http.Error (List Node) -> msg) -> Cmd msg
fetchNodes toMsg =
    Http.get
        { url = "/api/nodes"
        , expect = Http.expectJson toMsg (list nodeDecoder)
        }


fetchStatuses : (Result Http.Error (List NodeStatus) -> msg) -> Cmd msg
fetchStatuses toMsg =
    Http.get
        { url = "/api/status"
        , expect = Http.expectJson toMsg (list statusDecoder)
        }


createNode : NodeForm -> (Result Http.Error Node -> msg) -> Cmd msg
createNode form toMsg =
    Http.post
        { url = "/api/nodes"
        , body = Http.jsonBody (encodeNodeForm form)
        , expect = Http.expectJson toMsg (field "node" nodeDecoder)
        }


updateNode : String -> Node -> (Result Http.Error Node -> msg) -> Cmd msg
updateNode nodeId node toMsg =
    Http.request
        { method = "PUT"
        , headers = []
        , url = "/api/nodes/" ++ nodeId
        , body = Http.jsonBody (encodeNodeUpdate node)
        , expect = Http.expectJson toMsg (field "node" nodeDecoder)
        , timeout = Nothing
        , tracker = Nothing
        }


updateNodePosition : String -> Float -> Float -> (Result Http.Error Node -> msg) -> Cmd msg
updateNodePosition nodeId x y toMsg =
    Http.request
        { method = "PUT"
        , headers = []
        , url = "/api/nodes/" ++ nodeId
        , body = Http.jsonBody (encodePositionUpdate x y)
        , expect = Http.expectJson toMsg (field "node" nodeDecoder)
        , timeout = Nothing
        , tracker = Nothing
        }


deleteNode : String -> (Result Http.Error () -> msg) -> Cmd msg
deleteNode nodeId toMsg =
    Http.request
        { method = "DELETE"
        , headers = []
        , url = "/api/nodes/" ++ nodeId
        , body = Http.emptyBody
        , expect = Http.expectWhatever toMsg
        , timeout = Nothing
        , tracker = Nothing
        }


-- DATABASE API CALLS


getDatabases : String -> (Result Http.Error (List DatabaseInfo) -> msg) -> Cmd msg
getDatabases nodeId toMsg =
    Http.get
        { url = "/api/nodes/" ++ nodeId ++ "/databases"
        , expect = Http.expectJson toMsg (field "data" (list databaseInfoDecoder))
        }


getDatabaseSchema : String -> String -> (Result Http.Error (List ModelInfo) -> msg) -> Cmd msg
getDatabaseSchema nodeId database toMsg =
    Http.get
        { url = "/api/nodes/" ++ nodeId ++ "/databases/" ++ Url.percentEncode database ++ "/schema"
        , expect = Http.expectJson toMsg (field "data" (list modelInfoDecoder))
        }


getDatabaseCommits : String -> String -> (Result Http.Error (List CommitInfo) -> msg) -> Cmd msg
getDatabaseCommits nodeId database toMsg =
    Http.get
        { url = "/api/nodes/" ++ nodeId ++ "/databases/" ++ Url.percentEncode database ++ "/commits"
        , expect = Http.expectJson toMsg (field "data" (list commitInfoDecoder))
        }


getDatabaseRemotes : String -> String -> (Result Http.Error (List RemoteInfo) -> msg) -> Cmd msg
getDatabaseRemotes nodeId database toMsg =
    Http.get
        { url = "/api/nodes/" ++ nodeId ++ "/databases/" ++ Url.percentEncode database ++ "/remotes"
        , expect = Http.expectJson toMsg (field "data" (list remoteInfoDecoder))
        }


addRemote : String -> String -> String -> String -> (Result Http.Error () -> msg) -> Cmd msg
addRemote nodeId database remoteName remoteUrl toMsg =
    Http.post
        { url = "/api/nodes/" ++ nodeId ++ "/databases/" ++ Url.percentEncode database ++ "/remotes"
        , body = Http.jsonBody (encodeAddRemote remoteName remoteUrl)
        , expect = Http.expectWhatever toMsg
        }


deleteRemote : String -> String -> String -> (Result Http.Error () -> msg) -> Cmd msg
deleteRemote nodeId database remoteName toMsg =
    Http.request
        { method = "DELETE"
        , headers = []
        , url = "/api/nodes/" ++ nodeId ++ "/databases/" ++ Url.percentEncode database ++ "/remotes/" ++ remoteName
        , body = Http.emptyBody
        , expect = Http.expectWhatever toMsg
        , timeout = Nothing
        , tracker = Nothing
        }
