module Types exposing (..)

{-| Core type definitions for the TerminusDB Manager
-}


{-| Position on the canvas (in pixels)
-}
type alias Position =
    { x : Float
    , y : Float
    }


{-| Configuration for a TerminusDB instance node
-}
type alias Node =
    { id : String
    , label : String
    , host : String
    , port : Int
    , username : String
    , password : String
    , sshEnabled : Bool
    , positionX : Float
    , positionY : Float
    }


{-| Runtime status of a node
-}
type alias NodeStatus =
    { nodeId : String
    , online : Bool
    , databaseCount : Int
    , remotes : List RemoteInfo
    , lastCheck : String
    , error : Maybe String
    }


{-| Information about a remote connection
-}
type alias RemoteInfo =
    { database : String
    , remoteName : String
    , remoteUrl : String
    , targetNodeId : Maybe String
    }


{-| Canvas view state (pan and zoom)
-}
type alias CanvasView =
    { offsetX : Float
    , offsetY : Float
    , zoom : Float
    }


{-| Dragging state
-}
type DragState
    = NotDragging
    | DraggingNode String Position -- node ID and offset from mouse to node position
    | DraggingCanvas Position -- last mouse position


{-| Context menu state
-}
type alias ContextMenu =
    { position : Position
    , visible : Bool
    }


{-| Node form data for creating/editing nodes
-}
type alias NodeForm =
    { label : String
    , host : String
    , port : String
    , username : String
    , password : String
    , sshEnabled : Bool
    , positionX : Float
    , positionY : Float
    }


{-| Form modal state
-}
type FormModal
    = NoModal
    | CreateNodeModal NodeForm
    | EditNodeModal String NodeForm -- node ID and form data
