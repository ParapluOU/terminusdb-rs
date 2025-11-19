module Main exposing (main)

{-| TerminusDB Manager - Main application
-}

import Api
import Browser
import Canvas
import Html exposing (Html, div, button, input, label, text, h2, p)
import Html.Attributes exposing (class, style, type_, value, placeholder, checked)
import Html.Events exposing (onClick, onInput)
import Http
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
    , error : Maybe String
    }


init : () -> ( Model, Cmd Msg )
init _ =
    ( { nodes = []
      , statuses = []
      , canvasView =
            { offsetX = 400
            , offsetY = 300
            , zoom = 1.0
            }
      , dragState = NotDragging
      , contextMenu = { position = { x = 0, y = 0 }, visible = False }
      , formModal = NoModal
      , error = Nothing
      }
    , Cmd.batch
        [ Api.fetchNodes NodesLoaded
        , Api.fetchStatuses StatusesLoaded
        ]
    )


-- UPDATE


type Msg
    = NodesLoaded (Result Http.Error (List Node))
    | StatusesLoaded (Result Http.Error (List NodeStatus))
    | PollStatus Time.Posix
    | MouseDown String Position
    | MouseMove Position
    | MouseUp
    | Wheel Float
    | ContextMenu Position
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
    | DeleteNodeClicked String
    | NodeDeleted (Result Http.Error ())


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        NodesLoaded result ->
            case result of
                Ok nodes ->
                    ( { model | nodes = nodes, error = Nothing }, Cmd.none )

                Err err ->
                    ( { model | error = Just (httpErrorToString err) }, Cmd.none )

        StatusesLoaded result ->
            case result of
                Ok statuses ->
                    ( { model | statuses = statuses }, Cmd.none )

                Err _ ->
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

                        view =
                            model.canvasView

                        newView =
                            { view
                                | offsetX = view.offsetX + deltaX
                                , offsetY = view.offsetY + deltaY
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

        Wheel deltaY ->
            let
                view =
                    model.canvasView

                zoomDelta =
                    if deltaY < 0 then
                        1.1

                    else
                        0.9

                newZoom =
                    clamp 0.1 3.0 (view.zoom * zoomDelta)

                newView =
                    { view | zoom = newZoom }
            in
            ( { model | canvasView = newView }, Cmd.none )

        ContextMenu pos ->
            ( { model
                | contextMenu = { position = pos, visible = True }
              }
            , Cmd.none
            )

        CloseContextMenu ->
            ( { model
                | contextMenu = { position = { x = 0, y = 0 }, visible = False }
              }
            , Cmd.none
            )

        OpenCreateNodeForm canvasPos ->
            let
                -- Convert screen position to canvas position
                view =
                    model.canvasView

                x =
                    (canvasPos.x - view.offsetX) / view.zoom

                y =
                    (canvasPos.y - view.offsetY) / view.zoom

                form =
                    { label = "New Instance"
                    , host = "localhost"
                    , port = "6363"
                    , username = "admin"
                    , password = "root"
                    , sshEnabled = False
                    , positionX = x
                    , positionY = y
                    }
            in
            ( { model
                | formModal = CreateNodeModal form
                , contextMenu = { position = { x = 0, y = 0 }, visible = False }
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
            ( { model | formModal = updateFormField (\f -> { f | port = value }) model.formModal }, Cmd.none )

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

        DeleteNodeClicked nodeId ->
            ( model, Api.deleteNode nodeId NodeDeleted )

        NodeDeleted result ->
            case result of
                Ok () ->
                    ( model, Api.fetchNodes NodesLoaded )

                Err err ->
                    ( { model | error = Just (httpErrorToString err) }, Cmd.none )


-- HELPERS


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
            }
        , viewContextMenu model.contextMenu
        , viewFormModal model.formModal
        , viewError model.error
        ]


viewContextMenu : ContextMenu -> Html Msg
viewContextMenu menu =
    if menu.visible then
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
            ]
            [ div
                [ class "menu-item"
                , style "padding" "8px 16px"
                , style "cursor" "pointer"
                , onClick (OpenCreateNodeForm menu.position)
                ]
                [ text "Add Node" ]
            , div
                [ class "menu-item"
                , style "padding" "8px 16px"
                , style "cursor" "pointer"
                , onClick CloseContextMenu
                ]
                [ text "Cancel" ]
            ]

    else
        text ""


viewFormModal : FormModal -> Html Msg
viewFormModal formModal =
    case formModal of
        CreateNodeModal form ->
            viewNodeForm "Create Node" form

        EditNodeModal _ form ->
            viewNodeForm "Edit Node" form

        NoModal ->
            text ""


viewNodeForm : String -> NodeForm -> Html Msg
viewNodeForm title form =
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
            , style "max-width" "500px"
            , style "width" "90%"
            , Html.Events.stopPropagationOn "click" (Html.Events.succeed ( CloseContextMenu, True ))
            ]
            [ h2 [] [ text title ]
            , viewFormField "Label" "text" form.label UpdateFormLabel
            , viewFormField "Host" "text" form.host UpdateFormHost
            , viewFormField "Port" "text" form.port UpdateFormPort
            , viewFormField "Username" "text" form.username UpdateFormUsername
            , viewFormField "Password" "password" form.password UpdateFormPassword
            , viewCheckbox "SSH Enabled" form.sshEnabled UpdateFormSshEnabled
            , div [ style "margin-top" "24px", style "display" "flex", style "gap" "8px" ]
                [ button
                    [ onClick SubmitNodeForm
                    , style "flex" "1"
                    , style "padding" "8px 16px"
                    , style "background" "#4caf50"
                    , style "color" "white"
                    , style "border" "none"
                    , style "border-radius" "4px"
                    , style "cursor" "pointer"
                    ]
                    [ text "Create" ]
                , button
                    [ onClick CloseForm
                    , style "flex" "1"
                    , style "padding" "8px 16px"
                    , style "background" "#999"
                    , style "color" "white"
                    , style "border" "none"
                    , style "border-radius" "4px"
                    , style "cursor" "pointer"
                    ]
                    [ text "Cancel" ]
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
                ]
                [ text error ]

        Nothing ->
            text ""


-- SUBSCRIPTIONS


subscriptions : Model -> Sub Msg
subscriptions model =
    Time.every 2000 PollStatus
