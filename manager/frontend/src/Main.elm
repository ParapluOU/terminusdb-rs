module Main exposing (main)

{-| TerminusDB Manager - Main application
-}

import Api
import Browser
import Browser.Events
import Canvas
import Html exposing (Html, div, button, input, label, text, h2, p, span)
import Html.Attributes exposing (class, style, type_, value, placeholder, checked, min, max, step, disabled)
import Html.Events exposing (onClick, onInput, on)
import Http
import Json.Decode
import Time
import Types exposing (..)


-- MAIN


main : Program () Model Msg
main =
    Browser.element
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        }


-- MODEL


type alias Model =
    { nodes : List Node
    , statuses : List NodeStatus
    , canvasView : CanvasView
    , dragState : DragState
    , contextMenu : ContextMenu
    , formModal : FormModal
    , confirmDialog : ConfirmDialog
    , databasePopover : Maybe DatabasePopover
    , databaseView : Maybe DatabaseView
    , error : Maybe String
    }


init : () -> ( Model, Cmd Msg )
init _ =
    ( { nodes = []
      , statuses = []
      , canvasView =
            { offsetX = 0
            , offsetY = 0
            , zoom = 1.0
            }
      , dragState = NotDragging
      , contextMenu = { position = { x = 0, y = 0 }, visible = False, nodeId = Nothing }
      , formModal = NoModal
      , confirmDialog = NoDialog
      , databasePopover = Nothing
      , databaseView = Nothing
      , error = Nothing
      }
    , Cmd.batch
        [ Api.fetchNodes NodesLoaded
        , Api.fetchStatuses StatusesLoaded
        ]
    )


-- UPDATE


type Msg
    = NoOp
    | KeyPressed String
    | NodesLoaded (Result Http.Error (List Node))
    | StatusesLoaded (Result Http.Error (List NodeStatus))
    | PollStatus Time.Posix
    | MouseDown String Position
    | MouseMove Position
    | MouseUp
    | Wheel Float Float  -- deltaX and deltaY
    | ZoomChanged Float
    | ContextMenu Position
    | NodeContextMenu String Position
    | NodeClick String Position  -- New: Click on node to open database popover
    | CloseContextMenu
    | OpenCreateNodeForm Position
    | CloseForm
    | UpdateFormLabel String
    | UpdateFormHost String
    | UpdateFormPort String
    | UpdateFormUsername String
    | UpdateFormPassword String
    | UpdateFormSshEnabled Bool
    | SubmitNodeForm
    | NodeCreated (Result Http.Error Node)
    | NodeUpdated (Result Http.Error Node)
    | ShowDeleteConfirmation String
    | ConfirmDelete String
    | CancelDelete
    | NodeDeleted (Result Http.Error ())
    -- Database popover messages
    | DatabasesLoaded String Position (Result Http.Error (List DatabaseInfo))  -- nodeId, position, result
    | CloseDatabasePopover
    -- Database view messages
    | OpenDatabaseView String String  -- nodeId, database name
    | CloseDatabaseView
    | SwitchDatabaseTab DatabaseTab
    | DatabaseSchemaLoaded (Result Http.Error (List ModelInfo))
    | DatabaseCommitsLoaded (Result Http.Error (List CommitInfo))
    | DatabaseRemotesLoaded (Result Http.Error (List RemoteInfo))


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        NoOp ->
            ( model, Cmd.none )

        KeyPressed key ->
            -- Handle ESC key to close modal when database view is open
            if key == "Escape" && model.databaseView /= Nothing then
                ( { model | databaseView = Nothing }, Cmd.none )
            else
                ( model, Cmd.none )

        NodesLoaded result ->
            case result of
                Ok nodes ->
                    let
                        -- Calculate the center of all nodes to focus the viewport
                        newCanvasView =
                            centerViewportOnNodes nodes model.canvasView
                    in
                    ( { model
                        | nodes = nodes
                        , canvasView = newCanvasView
                        , error = Nothing
                      }
                    , Cmd.none
                    )

                Err err ->
                    ( { model | error = Just (httpErrorToString err) }, Cmd.none )

        StatusesLoaded result ->
            case result of
                Ok statuses ->
                    let
                        _ = Debug.log "Loaded statuses" statuses
                    in
                    ( { model | statuses = statuses }, Cmd.none )

                Err err ->
                    let
                        _ = Debug.log "Status load error" (httpErrorToString err)
                    in
                    ( model, Cmd.none )

        PollStatus _ ->
            ( model, Api.fetchStatuses StatusesLoaded )

        MouseDown nodeId pos ->
            let
                node =
                    findNode nodeId model.nodes

                dragState =
                    case node of
                        Just n ->
                            DraggingNode nodeId
                                { x = pos.x - n.positionX
                                , y = pos.y - n.positionY
                                }

                        Nothing ->
                            NotDragging
            in
            ( { model | dragState = dragState }, Cmd.none )

        MouseMove pos ->
            case model.dragState of
                DraggingNode nodeId offset ->
                    let
                        newX =
                            pos.x - offset.x

                        newY =
                            pos.y - offset.y

                        updatedNodes =
                            updateNodePosition nodeId newX newY model.nodes
                    in
                    ( { model | nodes = updatedNodes }, Cmd.none )

                DraggingCanvas lastPos ->
                    let
                        deltaX =
                            pos.x - lastPos.x

                        deltaY =
                            pos.y - lastPos.y

                        currentView =
                            model.canvasView

                        newView =
                            { currentView
                                | offsetX = currentView.offsetX + deltaX
                                , offsetY = currentView.offsetY + deltaY
                            }
                    in
                    ( { model
                        | canvasView = newView
                        , dragState = DraggingCanvas pos
                      }
                    , Cmd.none
                    )

                NotDragging ->
                    ( model, Cmd.none )

        MouseUp ->
            case model.dragState of
                DraggingNode nodeId _ ->
                    let
                        node =
                            findNode nodeId model.nodes

                        cmd =
                            case node of
                                Just n ->
                                    Api.updateNode n.id n NodeUpdated

                                Nothing ->
                                    Cmd.none
                    in
                    ( { model | dragState = NotDragging }, cmd )

                _ ->
                    ( { model | dragState = NotDragging }, Cmd.none )

        Wheel deltaX deltaY ->
            let
                currentView =
                    model.canvasView

                -- Pan the canvas based on wheel direction
                panSpeed =
                    60.0

                newView =
                    { currentView
                        | offsetX = currentView.offsetX - (deltaX * panSpeed / 100)
                        , offsetY = currentView.offsetY - (deltaY * panSpeed / 100)
                    }
            in
            ( { model | canvasView = newView }, Cmd.none )

        ZoomChanged zoom ->
            let
                currentView =
                    model.canvasView

                newView =
                    { currentView | zoom = zoom }
            in
            ( { model | canvasView = newView }, Cmd.none )

        ContextMenu pos ->
            ( { model
                | contextMenu = { position = pos, visible = True, nodeId = Nothing }
              }
            , Cmd.none
            )

        NodeContextMenu nodeId pos ->
            ( { model
                | contextMenu = { position = pos, visible = True, nodeId = Just nodeId }
              }
            , Cmd.none
            )

        CloseContextMenu ->
            ( { model
                | contextMenu = { position = { x = 0, y = 0 }, visible = False, nodeId = Nothing }
              }
            , Cmd.none
            )

        OpenCreateNodeForm canvasPos ->
            let
                -- Convert screen position to canvas position
                currentView =
                    model.canvasView

                x =
                    (canvasPos.x - currentView.offsetX) / currentView.zoom

                y =
                    (canvasPos.y - currentView.offsetY) / currentView.zoom

                form =
                    { label = "New Instance"
                    , host = "localhost"
                    , portNumber = "6363"
                    , username = "admin"
                    , password = "root"
                    , sshEnabled = False
                    , positionX = x
                    , positionY = y
                    }
            in
            ( { model
                | formModal = CreateNodeModal form
                , contextMenu = { position = { x = 0, y = 0 }, visible = False, nodeId = Nothing }
              }
            , Cmd.none
            )

        CloseForm ->
            ( { model | formModal = NoModal }, Cmd.none )

        UpdateFormLabel value ->
            ( { model | formModal = updateFormField (\f -> { f | label = value }) model.formModal }, Cmd.none )

        UpdateFormHost value ->
            ( { model | formModal = updateFormField (\f -> { f | host = value }) model.formModal }, Cmd.none )

        UpdateFormPort value ->
            ( { model | formModal = updateFormField (\f -> { f | portNumber = value }) model.formModal }, Cmd.none )

        UpdateFormUsername value ->
            ( { model | formModal = updateFormField (\f -> { f | username = value }) model.formModal }, Cmd.none )

        UpdateFormPassword value ->
            ( { model | formModal = updateFormField (\f -> { f | password = value }) model.formModal }, Cmd.none )

        UpdateFormSshEnabled value ->
            ( { model | formModal = updateFormField (\f -> { f | sshEnabled = value }) model.formModal }, Cmd.none )

        SubmitNodeForm ->
            case model.formModal of
                CreateNodeModal form ->
                    ( model, Api.createNode form NodeCreated )

                _ ->
                    ( model, Cmd.none )

        NodeCreated result ->
            case result of
                Ok node ->
                    ( { model
                        | nodes = node :: model.nodes
                        , formModal = NoModal
                        , error = Nothing
                      }
                    , Cmd.none
                    )

                Err err ->
                    ( { model | error = Just (httpErrorToString err) }, Cmd.none )

        NodeUpdated result ->
            case result of
                Ok updatedNode ->
                    let
                        updatedNodes =
                            model.nodes
                                |> List.map
                                    (\n ->
                                        if n.id == updatedNode.id then
                                            updatedNode

                                        else
                                            n
                                    )
                    in
                    ( { model | nodes = updatedNodes }, Cmd.none )

                Err _ ->
                    ( model, Cmd.none )

        ShowDeleteConfirmation nodeId ->
            ( { model
                | confirmDialog = DeleteNodeDialog nodeId
                , contextMenu = { position = { x = 0, y = 0 }, visible = False, nodeId = Nothing }
              }
            , Cmd.none
            )

        ConfirmDelete nodeId ->
            ( { model | confirmDialog = NoDialog }
            , Api.deleteNode nodeId NodeDeleted
            )

        CancelDelete ->
            ( { model | confirmDialog = NoDialog }, Cmd.none )

        NodeDeleted result ->
            case result of
                Ok () ->
                    ( model, Api.fetchNodes NodesLoaded )

                Err err ->
                    ( { model | error = Just (httpErrorToString err) }, Cmd.none )

        NodeClick nodeId pos ->
            -- Only fetch databases if node is accessible
            let
                nodeStatus = List.filter (\s -> s.nodeId == nodeId) model.statuses |> List.head
                isAccessible = case nodeStatus of
                    Just status -> status.connectivity == Accessible
                    Nothing -> False
            in
            if isAccessible then
                ( model
                , Api.getDatabases nodeId (DatabasesLoaded nodeId pos)
                )
            else
                -- Node is not accessible, don't open popover
                ( model, Cmd.none )

        DatabasesLoaded nodeId pos result ->
            case result of
                Ok databases ->
                    ( { model
                        | databasePopover = Just
                            { nodeId = nodeId
                            , position = pos
                            , databases = databases
                            }
                      }
                    , Cmd.none
                    )

                Err err ->
                    ( { model | error = Just (httpErrorToString err) }, Cmd.none )

        CloseDatabasePopover ->
            ( { model | databasePopover = Nothing }, Cmd.none )

        OpenDatabaseView nodeId database ->
            let
                initialDbView =
                    { nodeId = nodeId
                    , database = database
                    , activeTab = ModelsTab
                    , models = []
                    , commits = []
                    , remotes = []
                    }
            in
            ( { model
                | databaseView = Just initialDbView
                , databasePopover = Nothing
              }
            , Cmd.batch
                [ Api.getDatabaseSchema nodeId database DatabaseSchemaLoaded
                , Api.getDatabaseCommits nodeId database DatabaseCommitsLoaded
                , Api.getDatabaseRemotes nodeId database DatabaseRemotesLoaded
                ]
            )

        CloseDatabaseView ->
            ( { model | databaseView = Nothing }, Cmd.none )

        SwitchDatabaseTab tab ->
            case model.databaseView of
                Just dbView ->
                    ( { model | databaseView = Just { dbView | activeTab = tab } }
                    , Cmd.none
                    )

                Nothing ->
                    ( model, Cmd.none )

        DatabaseSchemaLoaded result ->
            case model.databaseView of
                Just dbView ->
                    case result of
                        Ok models ->
                            ( { model | databaseView = Just { dbView | models = models } }
                            , Cmd.none
                            )

                        Err err ->
                            ( { model | error = Just (httpErrorToString err) }, Cmd.none )

                Nothing ->
                    ( model, Cmd.none )

        DatabaseCommitsLoaded result ->
            case model.databaseView of
                Just dbView ->
                    case result of
                        Ok commits ->
                            ( { model | databaseView = Just { dbView | commits = commits } }
                            , Cmd.none
                            )

                        Err err ->
                            ( { model | error = Just (httpErrorToString err) }, Cmd.none )

                Nothing ->
                    ( model, Cmd.none )

        DatabaseRemotesLoaded result ->
            case model.databaseView of
                Just dbView ->
                    case result of
                        Ok remotes ->
                            ( { model | databaseView = Just { dbView | remotes = remotes } }
                            , Cmd.none
                            )

                        Err err ->
                            ( { model | error = Just (httpErrorToString err) }, Cmd.none )

                Nothing ->
                    ( model, Cmd.none )


-- HELPERS


centerViewportOnNodes : List Node -> CanvasView -> CanvasView
centerViewportOnNodes nodes currentView =
    if List.isEmpty nodes then
        currentView
    else
        let
            -- Node dimensions (from Canvas.elm)
            nodeWidth = 200
            nodeHeight = 120

            -- Calculate bounding box of all nodes
            positions =
                nodes |> List.map (\n -> (n.positionX, n.positionY))

            minX =
                positions
                    |> List.map Tuple.first
                    |> List.minimum
                    |> Maybe.withDefault 0

            maxX =
                positions
                    |> List.map Tuple.first
                    |> List.maximum
                    |> Maybe.withDefault 0

            minY =
                positions
                    |> List.map Tuple.second
                    |> List.minimum
                    |> Maybe.withDefault 0

            maxY =
                positions
                    |> List.map Tuple.second
                    |> List.maximum
                    |> Maybe.withDefault 0

            -- Calculate center of all nodes (accounting for node dimensions)
            centerX =
                (minX + maxX + nodeWidth) / 2

            centerY =
                (minY + maxY + nodeHeight) / 2

            -- Viewport center (assuming typical viewport size)
            viewportCenterX =
                960

            viewportCenterY =
                540

            -- Calculate offsets to center the nodes in the viewport
            offsetX =
                viewportCenterX - (centerX * currentView.zoom)

            offsetY =
                viewportCenterY - (centerY * currentView.zoom)
        in
        { currentView | offsetX = offsetX, offsetY = offsetY }


findNode : String -> List Node -> Maybe Node
findNode nodeId nodes =
    nodes
        |> List.filter (\n -> n.id == nodeId)
        |> List.head


updateNodePosition : String -> Float -> Float -> List Node -> List Node
updateNodePosition nodeId newX newY nodes =
    nodes
        |> List.map
            (\n ->
                if n.id == nodeId then
                    { n | positionX = newX, positionY = newY }

                else
                    n
            )


updateFormField : (NodeForm -> NodeForm) -> FormModal -> FormModal
updateFormField updater formModal =
    case formModal of
        CreateNodeModal form ->
            CreateNodeModal (updater form)

        EditNodeModal id form ->
            EditNodeModal id (updater form)

        NoModal ->
            NoModal


httpErrorToString : Http.Error -> String
httpErrorToString error =
    case error of
        Http.BadUrl url ->
            "Bad URL: " ++ url

        Http.Timeout ->
            "Request timeout"

        Http.NetworkError ->
            "Network error"

        Http.BadStatus status ->
            "Bad status: " ++ String.fromInt status

        Http.BadBody body ->
            "Bad body: " ++ body


-- VIEW


view : Model -> Html Msg
view model =
    div [ class "app" ]
        [ Canvas.view
            { nodes = model.nodes
            , statuses = model.statuses
            , canvasView = model.canvasView
            , dragState = model.dragState
            , onMouseDown = MouseDown
            , onMouseMove = MouseMove
            , onMouseUp = MouseUp
            , onWheel = Wheel
            , onContextMenu = ContextMenu
            , onNodeContextMenu = NodeContextMenu
            , onNodeClick = NodeClick
            }
        , viewZoomControl model.canvasView.zoom
        , viewContextMenu model.contextMenu model.nodes
        , viewFormModal model.formModal model.nodes
        , viewConfirmDialog model.confirmDialog model.nodes
        , viewDatabasePopover model.canvasView model.databasePopover
        , viewDatabaseView model.databaseView
        , viewError model.error
        ]


viewContextMenu : ContextMenu -> List Node -> Html Msg
viewContextMenu menu nodes =
    if menu.visible then
        div []
            [ -- Invisible overlay to capture clicks outside the menu
              div
                [ class "context-menu-overlay"
                , style "position" "fixed"
                , style "top" "0"
                , style "left" "0"
                , style "right" "0"
                , style "bottom" "0"
                , style "z-index" "999"
                , onClick CloseContextMenu
                ]
                []
            , -- The actual context menu
              div
                [ class "context-menu"
                , style "position" "absolute"
                , style "left" (String.fromFloat menu.position.x ++ "px")
                , style "top" (String.fromFloat menu.position.y ++ "px")
                , style "background" "white"
                , style "border" "1px solid #ccc"
                , style "border-radius" "4px"
                , style "box-shadow" "0 2px 8px rgba(0,0,0,0.15)"
                , style "padding" "8px 0"
                , style "z-index" "1000"
                , Html.Events.stopPropagationOn "click" (Json.Decode.succeed ( CloseContextMenu, True ))
                ]
                (case menu.nodeId of
                    Just nodeId ->
                        let
                            nodeLabel =
                                findNode nodeId nodes
                                    |> Maybe.map .label
                                    |> Maybe.withDefault "Node"

                            isLocal =
                                nodeId == "local"
                        in
                        if isLocal then
                            [ div
                                [ class "menu-item"
                                , style "padding" "8px 16px"
                                , style "color" "#999"
                                , style "font-style" "italic"
                                ]
                                [ text "Cannot delete local instance" ]
                            ]
                        else
                            [ div
                                [ class "menu-item"
                                , style "padding" "8px 16px"
                                , style "cursor" "pointer"
                                , style "color" "#f44336"
                                , style "display" "flex"
                                , style "align-items" "center"
                                , style "gap" "8px"
                                , onClick (ShowDeleteConfirmation nodeId)
                                ]
                                [ span [] [ text "🗑️" ]
                                , text ("Delete \"" ++ nodeLabel ++ "\"...")
                                ]
                            ]

                    Nothing ->
                        [ div
                            [ class "menu-item"
                            , style "padding" "8px 16px"
                            , style "cursor" "pointer"
                            , onClick (OpenCreateNodeForm menu.position)
                            ]
                            [ text "Add Node" ]
                        ]
                )
            ]

    else
        text ""


viewFormModal : FormModal -> List Node -> Html Msg
viewFormModal formModal existingNodes =
    case formModal of
        CreateNodeModal form ->
            viewNodeForm "Create Node" form existingNodes

        EditNodeModal _ form ->
            viewNodeForm "Edit Node" form existingNodes

        NoModal ->
            text ""


viewNodeForm : String -> NodeForm -> List Node -> Html Msg
viewNodeForm title form existingNodes =
    let
        -- Check for duplicate host/port combination
        portInt =
            String.toInt form.portNumber |> Maybe.withDefault 0

        isDuplicate =
            existingNodes
                |> List.any (\node -> node.host == form.host && node.portNumber == portInt)

        hasValidationError =
            isDuplicate
    in
    div
        [ class "modal-overlay"
        , style "position" "fixed"
        , style "top" "0"
        , style "left" "0"
        , style "right" "0"
        , style "bottom" "0"
        , style "background" "rgba(0,0,0,0.5)"
        , style "display" "flex"
        , style "align-items" "center"
        , style "justify-content" "center"
        , style "z-index" "2000"
        , onClick CloseForm
        ]
        [ div
            [ class "modal-content"
            , style "background" "white"
            , style "padding" "24px"
            , style "border-radius" "8px"
            , style "max-width" "700px"
            , style "width" "90%"
            , style "position" "relative"
            , Html.Events.stopPropagationOn "click" (Json.Decode.succeed ( CloseContextMenu, True ))
            ]
            [ -- Close button (X)
              button
                [ onClick CloseForm
                , style "position" "absolute"
                , style "top" "16px"
                , style "right" "16px"
                , style "background" "transparent"
                , style "border" "none"
                , style "font-size" "24px"
                , style "cursor" "pointer"
                , style "color" "#666"
                , style "line-height" "1"
                , style "padding" "0"
                , style "width" "32px"
                , style "height" "32px"
                ]
                [ text "×" ]
            , h2 [] [ text title ]
            , div
                [ style "display" "grid"
                , style "grid-template-columns" "1fr 1fr"
                , style "gap" "16px"
                , style "margin-top" "16px"
                ]
                [ -- Left column
                  div []
                    [ viewFormField "Label" "text" form.label UpdateFormLabel
                    , viewFormField "Host" "text" form.host UpdateFormHost
                    , viewFormField "Port" "text" form.portNumber UpdateFormPort
                    ]
                , -- Right column
                  div []
                    [ viewFormField "Username" "text" form.username UpdateFormUsername
                    , viewFormField "Password" "password" form.password UpdateFormPassword
                    , viewCheckbox "SSH Enabled" form.sshEnabled UpdateFormSshEnabled
                    ]
                ]
            , -- Validation error message
              if isDuplicate then
                div
                    [ style "margin-top" "16px"
                    , style "padding" "12px"
                    , style "background" "#ffebee"
                    , style "border" "1px solid #f44336"
                    , style "border-radius" "4px"
                    , style "color" "#c62828"
                    , style "font-size" "14px"
                    ]
                    [ text ("⚠️ A node with host \"" ++ form.host ++ ":" ++ form.portNumber ++ "\" already exists") ]
              else
                text ""
            , div [ style "margin-top" "24px" ]
                [ button
                    [ onClick SubmitNodeForm
                    , style "width" "100%"
                    , style "padding" "12px 16px"
                    , style "background" (if hasValidationError then "#cccccc" else "#4caf50")
                    , style "color" "white"
                    , style "border" "none"
                    , style "border-radius" "4px"
                    , style "cursor" (if hasValidationError then "not-allowed" else "pointer")
                    , style "font-size" "16px"
                    , style "font-weight" "500"
                    , Html.Attributes.disabled hasValidationError
                    ]
                    [ text "Create Node" ]
                ]
            ]
        ]


viewFormField : String -> String -> String -> (String -> Msg) -> Html Msg
viewFormField labelText inputType val onChange =
    div [ style "margin-bottom" "16px" ]
        [ label [ style "display" "block", style "margin-bottom" "4px", style "font-weight" "500" ]
            [ text labelText ]
        , input
            [ type_ inputType
            , value val
            , onInput onChange
            , style "width" "100%"
            , style "padding" "8px"
            , style "border" "1px solid #ddd"
            , style "border-radius" "4px"
            ]
            []
        ]


viewCheckbox : String -> Bool -> (Bool -> Msg) -> Html Msg
viewCheckbox labelText val onChange =
    div [ style "margin-bottom" "16px" ]
        [ label [ style "display" "flex", style "align-items" "center", style "cursor" "pointer" ]
            [ input
                [ type_ "checkbox"
                , checked val
                , Html.Events.onCheck onChange
                , style "margin-right" "8px"
                ]
                []
            , text labelText
            ]
        ]


viewZoomControl : Float -> Html Msg
viewZoomControl zoom =
    div
        [ style "position" "fixed"
        , style "bottom" "20px"
        , style "left" "20px"
        , style "background" "white"
        , style "padding" "12px 16px"
        , style "border-radius" "8px"
        , style "box-shadow" "0 2px 8px rgba(0,0,0,0.15)"
        , style "display" "flex"
        , style "align-items" "center"
        , style "gap" "12px"
        , style "z-index" "100"
        ]
        [ span
            [ style "font-size" "14px"
            , style "color" "#666"
            , style "min-width" "40px"
            ]
            [ text "Zoom:" ]
        , input
            [ type_ "range"
            , Html.Attributes.min "0.1"
            , Html.Attributes.max "3.0"
            , step "0.1"
            , value (String.fromFloat zoom)
            , on "input"
                (Json.Decode.map
                    (\str ->
                        ZoomChanged
                            (String.toFloat str |> Maybe.withDefault 1.0)
                    )
                    Html.Events.targetValue
                )
            , style "width" "150px"
            ]
            []
        , span
            [ style "font-size" "14px"
            , style "color" "#333"
            , style "min-width" "45px"
            , style "font-weight" "bold"
            ]
            [ text (String.fromInt (round (zoom * 100)) ++ "%") ]
        ]


viewError : Maybe String -> Html Msg
viewError maybeError =
    case maybeError of
        Just error ->
            div
                [ style "position" "fixed"
                , style "bottom" "20px"
                , style "right" "20px"
                , style "background" "#f44336"
                , style "color" "white"
                , style "padding" "16px"
                , style "border-radius" "4px"
                , style "box-shadow" "0 2px 8px rgba(0,0,0,0.2)"
                , style "max-width" "400px"
                , style "z-index" "3000"
                ]
                [ text error ]

        Nothing ->
            text ""


viewConfirmDialog : ConfirmDialog -> List Node -> Html Msg
viewConfirmDialog dialog nodes =
    case dialog of
        DeleteNodeDialog nodeId ->
            let
                nodeLabel =
                    findNode nodeId nodes
                        |> Maybe.map .label
                        |> Maybe.withDefault "Node"
            in
            div
                [ class "modal-overlay"
                , style "position" "fixed"
                , style "top" "0"
                , style "left" "0"
                , style "right" "0"
                , style "bottom" "0"
                , style "background" "rgba(0,0,0,0.5)"
                , style "display" "flex"
                , style "align-items" "center"
                , style "justify-content" "center"
                , style "z-index" "2000"
                , onClick CancelDelete
                ]
                [ div
                    [ class "confirm-dialog"
                    , style "background" "white"
                    , style "padding" "24px"
                    , style "border-radius" "8px"
                    , style "max-width" "500px"
                    , style "width" "90%"
                    , Html.Events.stopPropagationOn "click" (Json.Decode.succeed ( CloseContextMenu, True ))
                    ]
                    [ h2
                        [ style "margin" "0 0 16px 0"
                        , style "color" "#f44336"
                        ]
                        [ text "⚠️ Confirm Deletion" ]
                    , p
                        [ style "margin" "0 0 24px 0"
                        , style "color" "#666"
                        ]
                        [ text "Are you sure you want to delete "
                        , span [ style "font-weight" "bold" ] [ text ("\"" ++ nodeLabel ++ "\"") ]
                        , text "? This action cannot be undone."
                        ]
                    , div
                        [ style "display" "flex"
                        , style "gap" "12px"
                        , style "justify-content" "flex-end"
                        ]
                        [ button
                            [ onClick CancelDelete
                            , style "padding" "10px 20px"
                            , style "background" "#e0e0e0"
                            , style "color" "#333"
                            , style "border" "none"
                            , style "border-radius" "4px"
                            , style "cursor" "pointer"
                            , style "font-size" "14px"
                            ]
                            [ text "Cancel" ]
                        , button
                            [ onClick (ConfirmDelete nodeId)
                            , style "padding" "10px 20px"
                            , style "background" "#f44336"
                            , style "color" "white"
                            , style "border" "none"
                            , style "border-radius" "4px"
                            , style "cursor" "pointer"
                            , style "font-size" "14px"
                            , style "font-weight" "500"
                            ]
                            [ text "Delete" ]
                        ]
                    ]
                ]

        NoDialog ->
            text ""


viewDatabasePopover : CanvasView -> Maybe DatabasePopover -> Html Msg
viewDatabasePopover canvasView maybePopover =
    case maybePopover of
        Just popover ->
            let
                -- Transform popover position to account for canvas pan and zoom
                transformedX = popover.position.x * canvasView.zoom + canvasView.offsetX
                transformedY = popover.position.y * canvasView.zoom + canvasView.offsetY
            in
            div []
                [ -- Invisible overlay - allows events to pass through to canvas
                  div
                    [ style "position" "fixed"
                    , style "top" "0"
                    , style "left" "0"
                    , style "right" "0"
                    , style "bottom" "0"
                    , style "z-index" "1100"
                    , style "pointer-events" "none"
                    ]
                    []
                , -- The actual popover
                  div
                    [ style "position" "absolute"
                    , style "left" (String.fromFloat transformedX ++ "px")
                    , style "top" (String.fromFloat transformedY ++ "px")
                    , style "background" "white"
                    , style "border" "1px solid #ccc"
                    , style "border-radius" "8px"
                    , style "box-shadow" "0 4px 12px rgba(0,0,0,0.15)"
                    , style "padding" "16px"
                    , style "z-index" "1200"
                    , style "pointer-events" "auto"
                    , style "min-width" "300px"
                    , style "max-width" "400px"
                    , style "max-height" "500px"
                    , style "overflow-y" "auto"
                    ]
                    [ -- Header with close button
                      div
                        [ style "display" "flex"
                        , style "justify-content" "space-between"
                        , style "align-items" "center"
                        , style "margin-bottom" "12px"
                        ]
                        [ h2
                            [ style "margin" "0"
                            , style "font-size" "18px"
                            , style "color" "#333"
                            ]
                            [ text "Databases" ]
                        , button
                            [ onClick CloseDatabasePopover
                            , style "background" "transparent"
                            , style "border" "none"
                            , style "font-size" "20px"
                            , style "cursor" "pointer"
                            , style "color" "#666"
                            , style "line-height" "1"
                            , style "padding" "0"
                            , style "width" "24px"
                            , style "height" "24px"
                            ]
                            [ text "×" ]
                        ]
                    , if List.isEmpty popover.databases then
                        div
                            [ style "color" "#999"
                            , style "font-style" "italic"
                            , style "padding" "20px 0"
                            , style "text-align" "center"
                            ]
                            [ text "No databases found" ]
                      else
                        div [] (List.map (viewDatabaseItem popover.nodeId) popover.databases)
                    ]
                ]

        Nothing ->
            text ""


viewDatabaseItem : String -> DatabaseInfo -> Html Msg
viewDatabaseItem nodeId db =
    div
        [ style "padding" "12px"
        , style "border-bottom" "1px solid #eee"
        , style "cursor" "pointer"
        , style "transition" "background 0.2s"
        , onClick (OpenDatabaseView nodeId db.name)
        ]
        [ div
            [ style "font-weight" "500"
            , style "color" "#333"
            , style "margin-bottom" "6px"
            ]
            [ text db.name ]
        , div
            [ style "display" "flex"
            , style "gap" "16px"
            , style "font-size" "12px"
            , style "color" "#666"
            ]
            [ span []
                [ text ("📊 " ++ String.fromInt db.commitCount ++ " commits") ]
            , span []
                [ text ("🔗 " ++ String.fromInt db.remoteCount ++ " remotes") ]
            ]
        , div
            [ style "font-size" "11px"
            , style "color" "#999"
            , style "margin-top" "4px"
            ]
            [ text ("Modified: " ++ db.lastModified) ]
        ]


viewDatabaseView : Maybe DatabaseView -> Html Msg
viewDatabaseView maybeView =
    case maybeView of
        Just dbView ->
            div
                [ class "modal-overlay"
                , style "position" "fixed"
                , style "top" "0"
                , style "left" "0"
                , style "right" "0"
                , style "bottom" "0"
                , style "background" "rgba(0,0,0,0.5)"
                , style "display" "flex"
                , style "align-items" "center"
                , style "justify-content" "center"
                , style "z-index" "2000"
                , onClick CloseDatabaseView
                ]
                [ div
                    [ class "modal-content"
                    , style "background" "white"
                    , style "border-radius" "8px"
                    , style "width" "90%"
                    , style "max-width" "1200px"
                    , style "height" "80vh"
                    , style "display" "flex"
                    , style "flex-direction" "column"
                    , Html.Events.stopPropagationOn "click" (Json.Decode.succeed ( NoOp, True ))
                    ]
                    [ -- Header
                      div
                        [ style "padding" "24px"
                        , style "border-bottom" "1px solid #eee"
                        , style "display" "flex"
                        , style "justify-content" "space-between"
                        , style "align-items" "center"
                        ]
                        [ h2
                            [ style "margin" "0"
                            , style "font-size" "24px"
                            ]
                            [ text dbView.database ]
                        , button
                            [ onClick CloseDatabaseView
                            , style "background" "transparent"
                            , style "border" "none"
                            , style "font-size" "24px"
                            , style "cursor" "pointer"
                            , style "color" "#666"
                            , style "line-height" "1"
                            , style "padding" "0"
                            , style "width" "32px"
                            , style "height" "32px"
                            ]
                            [ text "×" ]
                        ]
                    , -- Tabs
                      div
                        [ style "display" "flex"
                        , style "border-bottom" "1px solid #eee"
                        , style "padding" "0 24px"
                        ]
                        [ viewTab dbView.activeTab ModelsTab "Models" (List.length dbView.models)
                        , viewTab dbView.activeTab CommitsTab "Commits" (List.length dbView.commits)
                        , viewTab dbView.activeTab RemotesTab "Remotes" (List.length dbView.remotes)
                        ]
                    , -- Content
                      div
                        [ style "flex" "1"
                        , style "overflow-y" "auto"
                        , style "padding" "24px"
                        ]
                        [ case dbView.activeTab of
                            ModelsTab ->
                                viewModelsTab dbView.models

                            CommitsTab ->
                                viewCommitsTab dbView.commits

                            RemotesTab ->
                                viewRemotesTab dbView.nodeId dbView.database dbView.remotes
                        ]
                    ]
                ]

        Nothing ->
            text ""


viewTab : DatabaseTab -> DatabaseTab -> String -> Int -> Html Msg
viewTab activeTab tab label count =
    let
        isActive =
            activeTab == tab

        activeStyles =
            if isActive then
                [ style "border-bottom" "2px solid #4caf50"
                , style "color" "#4caf50"
                ]
            else
                []
    in
    button
        ([ onClick (SwitchDatabaseTab tab)
         , style "padding" "12px 16px"
         , style "background" "transparent"
         , style "border" "none"
         , style "cursor" "pointer"
         , style "font-size" "14px"
         , style "font-weight" (if isActive then "600" else "400")
         , style "color" (if isActive then "#4caf50" else "#666")
         , style "transition" "all 0.2s"
         ]
            ++ activeStyles
        )
        [ text (label ++ " (" ++ String.fromInt count ++ ")") ]


viewModelsTab : List ModelInfo -> Html Msg
viewModelsTab models =
    if List.isEmpty models then
        div
            [ style "text-align" "center"
            , style "padding" "40px"
            , style "color" "#999"
            , style "font-style" "italic"
            ]
            [ text "No models found" ]
    else
        div []
            [ div
                [ style "display" "grid"
                , style "grid-template-columns" "1fr auto"
                , style "gap" "8px"
                , style "padding" "8px 0"
                , style "border-bottom" "2px solid #eee"
                , style "font-weight" "600"
                , style "color" "#666"
                ]
                [ text "Model Name"
                , text "Instances"
                ]
            , div [] (List.map viewModelRow models)
            ]


viewModelRow : ModelInfo -> Html Msg
viewModelRow model =
    div
        [ style "display" "grid"
        , style "grid-template-columns" "1fr auto"
        , style "gap" "8px"
        , style "padding" "12px 0"
        , style "border-bottom" "1px solid #eee"
        ]
        [ div
            [ style "font-family" "monospace"
            , style "color" "#333"
            ]
            [ text model.name ]
        , div
            [ style "color" "#666"
            , style "text-align" "right"
            ]
            [ text (String.fromInt model.instanceCount) ]
        ]


viewCommitsTab : List CommitInfo -> Html Msg
viewCommitsTab commits =
    if List.isEmpty commits then
        div
            [ style "text-align" "center"
            , style "padding" "40px"
            , style "color" "#999"
            , style "font-style" "italic"
            ]
            [ text "No commits found" ]
    else
        div [] (List.map viewCommitRow commits)


viewCommitRow : CommitInfo -> Html Msg
viewCommitRow commit =
    div
        [ style "padding" "16px"
        , style "border-bottom" "1px solid #eee"
        ]
        [ div
            [ style "font-family" "monospace"
            , style "font-size" "12px"
            , style "color" "#666"
            , style "margin-bottom" "4px"
            ]
            [ text commit.id ]
        , div
            [ style "font-weight" "500"
            , style "color" "#333"
            , style "margin-bottom" "4px"
            ]
            [ text commit.message ]
        , div
            [ style "font-size" "12px"
            , style "color" "#999"
            ]
            [ text (commit.author ++ " • " ++ commit.timestamp) ]
        ]


viewRemotesTab : String -> String -> List RemoteInfo -> Html Msg
viewRemotesTab nodeId database remotes =
    div []
        [ div
            [ style "margin-bottom" "16px"
            , style "display" "flex"
            , style "justify-content" "flex-end"
            ]
            [ button
                [ style "padding" "8px 16px"
                , style "background" "#4caf50"
                , style "color" "white"
                , style "border" "none"
                , style "border-radius" "4px"
                , style "cursor" "pointer"
                , style "font-size" "14px"
                ]
                [ text "+ Add Remote" ]
            ]
        , if List.isEmpty remotes then
            div
                [ style "text-align" "center"
                , style "padding" "40px"
                , style "color" "#999"
                , style "font-style" "italic"
                ]
                [ text "No remotes configured" ]
          else
            div [] (List.map viewRemoteRow remotes)
        ]


viewRemoteRow : RemoteInfo -> Html Msg
viewRemoteRow remote =
    div
        [ style "padding" "16px"
        , style "border" "1px solid #eee"
        , style "border-radius" "4px"
        , style "margin-bottom" "12px"
        , style "display" "flex"
        , style "justify-content" "space-between"
        , style "align-items" "center"
        ]
        [ div []
            [ div
                [ style "font-weight" "600"
                , style "color" "#333"
                , style "margin-bottom" "4px"
                ]
                [ text remote.remoteName ]
            , div
                [ style "font-family" "monospace"
                , style "font-size" "12px"
                , style "color" "#666"
                ]
                [ text remote.remoteUrl ]
            ]
        , button
            [ style "padding" "6px 12px"
            , style "background" "#f44336"
            , style "color" "white"
            , style "border" "none"
            , style "border-radius" "4px"
            , style "cursor" "pointer"
            , style "font-size" "12px"
            ]
            [ text "Delete" ]
        ]


-- SUBSCRIPTIONS


subscriptions : Model -> Sub Msg
subscriptions model =
    Sub.batch
        [ Time.every 2000 PollStatus
        , Browser.Events.onKeyDown (Json.Decode.map KeyPressed (Json.Decode.field "key" Json.Decode.string))
        ]
