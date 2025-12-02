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
    , portNumber : Int
    , username : String
    , password : String
    , sshEnabled : Bool
    , positionX : Float
    , positionY : Float
    }


{-| Connectivity level for a node
-}
type Connectivity
    = Unreachable
    | Reachable
    | Accessible


{-| Runtime status of a node
-}
type alias NodeStatus =
    { nodeId : String
    , online : Bool
    , connectivity : Connectivity
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
    , nodeId : Maybe String  -- If right-clicked on a node, this will be the node ID
    }


{-| Node form data for creating/editing nodes
-}
type alias NodeForm =
    { label : String
    , host : String
    , portNumber : String
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


{-| Confirmation dialog state
-}
type ConfirmDialog
    = NoDialog
    | DeleteNodeDialog String  -- node ID to delete


{-| Database information with metadata
-}
type alias DatabaseInfo =
    { name : String
    , commitCount : Int
    , lastModified : String
    , remoteCount : Int
    }


{-| Model/Entity type information
-}
type alias ModelInfo =
    { name : String
    , instanceCount : Int
    }


{-| Commit information
-}
type alias CommitInfo =
    { id : String
    , author : String
    , message : String
    , timestamp : String
    }


{-| Database popover state (shown when clicking a node)
-}
type alias DatabasePopover =
    { nodeId : String
    , position : Position
    , databases : List DatabaseInfo
    }


{-| Database view modal state
-}
type alias DatabaseView =
    { nodeId : String
    , database : String
    , activeTab : DatabaseTab
    , models : List ModelInfo
    , commits : List CommitInfo
    , remotes : List RemoteInfo
    }


{-| Database view tabs
-}
type DatabaseTab
    = ModelsTab
    | CommitsTab
    | RemotesTab


{-| Remote dialog state
-}
type RemoteDialog
    = NoRemoteDialog
    | AddRemoteDialog String String -- nodeId, database
        { remoteName : String
        , remoteUrl : String
        }
