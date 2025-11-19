module Api exposing
    ( fetchNodes
    , fetchStatuses
    , createNode
    , updateNode
    , deleteNode
    , nodeDecoder
    , statusDecoder
    )

{-| HTTP API client for TerminusDB Manager backend
-}

import Http
import Json.Decode as Decode exposing (Decoder, field, string, int, bool, float, list, maybe)
import Json.Encode as Encode
import Types exposing (Node, NodeStatus, RemoteInfo, NodeForm)


-- DECODERS


nodeDecoder : Decoder Node
nodeDecoder =
    Decode.map9 Node
        (field "id" string)
        (field "label" string)
        (field "host" string)
        (field "port" int)
        (field "username" string)
        (field "password" string)
        (field "ssh_enabled" bool)
        (field "position_x" float)
        (field "position_y" float)


remoteInfoDecoder : Decoder RemoteInfo
remoteInfoDecoder =
    Decode.map4 RemoteInfo
        (field "database" string)
        (field "remote_name" string)
        (field "remote_url" string)
        (field "target_node_id" (maybe string))


statusDecoder : Decoder NodeStatus
statusDecoder =
    Decode.map6 NodeStatus
        (field "node_id" string)
        (field "online" bool)
        (field "database_count" int)
        (field "remotes" (list remoteInfoDecoder))
        (field "last_check" string)
        (field "error" (maybe string))


-- ENCODERS


encodeNodeForm : NodeForm -> Encode.Value
encodeNodeForm form =
    Encode.object
        [ ( "label", Encode.string form.label )
        , ( "host", Encode.string form.host )
        , ( "port", Encode.int (Maybe.withDefault 6363 (String.toInt form.port)) )
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
        , ( "port", Encode.int node.port )
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
