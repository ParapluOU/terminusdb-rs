module Canvas exposing (view)

{-| SVG canvas for rendering and interacting with nodes
-}

import Html exposing (Html, div)
import Html.Attributes exposing (class, style)
import Html.Events
import Json.Decode as Decode
import Svg exposing (Svg, svg, g, rect, line, text, text_)
import Svg.Attributes as SA
import Types exposing (..)


-- VIEW


view :
    { nodes : List Node
    , statuses : List NodeStatus
    , canvasView : CanvasView
    , dragState : DragState
    , onMouseDown : String -> Position -> msg
    , onMouseMove : Position -> msg
    , onMouseUp : msg
    , onWheel : Float -> msg
    , onContextMenu : Position -> msg
    }
    -> Html msg
view config =
    div
        [ class "canvas-container"
        , style "width" "100%"
        , style "height" "100vh"
        , style "overflow" "hidden"
        , style "position" "relative"
        , style "background" "#f0f0f0"
        ]
        [ svg
            [ SA.width "100%"
            , SA.height "100%"
            , onMouseMove config.onMouseMove
            , onMouseUp config.onMouseUp
            , onWheel config.onWheel
            , preventContextMenu config.onContextMenu
            ]
            [ -- SVG definitions (shadows, markers)
              Svg.defs []
                [ -- Drop shadow for nodes
                  Svg.filter [ SA.id "shadow" ]
                    [ Svg.feDropShadow
                        [ SA.dx "0"
                        , SA.dy "2"
                        , SA.stdDeviation "4"
                        , SA.floodOpacity "0.2"
                        ]
                        []
                    ]

                , -- Arrow marker for remote connections
                  Svg.marker
                    [ SA.id "arrowhead"
                    , SA.markerWidth "10"
                    , SA.markerHeight "10"
                    , SA.refX "9"
                    , SA.refY "3"
                    , SA.orient "auto"
                    , SA.markerUnits "strokeWidth"
                    ]
                    [ Svg.path
                        [ SA.d "M0,0 L0,6 L9,3 z"
                        , SA.fill "#2196f3"
                        ]
                        []
                    ]
                ]

            , -- Background grid (optional)
              viewGrid config.canvasView

            , -- Transform group for pan/zoom
              g
                [ SA.transform
                    (transformString config.canvasView)
                ]
                [ -- Remote connection lines
                  g [] (viewRemoteLines config.nodes config.statuses)

                , -- Nodes
                  g [] (List.map (viewNode config.statuses config.onMouseDown) config.nodes)
                ]
            ]
        ]


transformString : CanvasView -> String
transformString view =
    "translate(" ++ String.fromFloat view.offsetX ++ "," ++ String.fromFloat view.offsetY ++ ") scale(" ++ String.fromFloat view.zoom ++ ")"


-- GRID


viewGrid : CanvasView -> Svg msg
viewGrid view =
    let
        gridSize =
            50

        gridOpacity =
            "0.1"
    in
    g []
        [ -- Vertical lines
          g []
            (List.range -20 20
                |> List.map
                    (\i ->
                        line
                            [ SA.x1 (String.fromFloat (toFloat i * gridSize))
                            , SA.y1 "-1000"
                            , SA.x2 (String.fromFloat (toFloat i * gridSize))
                            , SA.y2 "1000"
                            , SA.stroke "#ccc"
                            , SA.strokeWidth "1"
                            , SA.opacity gridOpacity
                            ]
                            []
                    )
            )

        , -- Horizontal lines
          g []
            (List.range -20 20
                |> List.map
                    (\i ->
                        line
                            [ SA.x1 "-1000"
                            , SA.y1 (String.fromFloat (toFloat i * gridSize))
                            , SA.x2 "1000"
                            , SA.y2 (String.fromFloat (toFloat i * gridSize))
                            , SA.stroke "#ccc"
                            , SA.strokeWidth "1"
                            , SA.opacity gridOpacity
                            ]
                            []
                    )
            )
        ]


-- NODES


viewNode : List NodeStatus -> (String -> Position -> msg) -> Node -> Svg msg
viewNode statuses onMouseDown node =
    let
        status =
            findStatus node.id statuses

        ( fillColor, strokeColor ) =
            case status of
                Just s ->
                    if s.online then
                        ( "#ffffff", "#4caf50" )

                    else
                        ( "#ffffff", "#f44336" )

                Nothing ->
                    ( "#ffffff", "#999999" )

        databaseCount =
            status
                |> Maybe.map .databaseCount
                |> Maybe.withDefault 0
    in
    g
        [ SA.transform ("translate(" ++ String.fromFloat node.positionX ++ "," ++ String.fromFloat node.positionY ++ ")")
        , SA.class "node"
        , SA.cursor "move"
        , onNodeMouseDown node.id onMouseDown
        ]
        [ -- Node background
          rect
            [ SA.width "200"
            , SA.height "100"
            , SA.rx "8"
            , SA.fill fillColor
            , SA.stroke strokeColor
            , SA.strokeWidth "3"
            , SA.filter "url(#shadow)"
            ]
            []

        , -- Status indicator (circle)
          Svg.circle
            [ SA.cx "20"
            , SA.cy "20"
            , SA.r "8"
            , SA.fill strokeColor
            ]
            []

        , -- Label
          text_
            [ SA.x "35"
            , SA.y "25"
            , SA.fontSize "16"
            , SA.fontWeight "bold"
            , SA.fill "#333"
            ]
            [ text node.label ]

        , -- Host:port
          text_
            [ SA.x "20"
            , SA.y "50"
            , SA.fontSize "12"
            , SA.fill "#666"
            ]
            [ text (node.host ++ ":" ++ String.fromInt node.port) ]

        , -- Database count
          text_
            [ SA.x "20"
            , SA.y "70"
            , SA.fontSize "12"
            , SA.fill "#666"
            ]
            [ text ("📊 " ++ String.fromInt databaseCount ++ " database" ++ (if databaseCount == 1 then "" else "s")) ]
        ]


findStatus : String -> List NodeStatus -> Maybe NodeStatus
findStatus nodeId statuses =
    statuses
        |> List.filter (\s -> s.nodeId == nodeId)
        |> List.head


-- REMOTE LINES


viewRemoteLines : List Node -> List NodeStatus -> List (Svg msg)
viewRemoteLines nodes statuses =
    statuses
        |> List.concatMap (viewNodeRemoteLines nodes)


viewNodeRemoteLines : List Node -> NodeStatus -> List (Svg msg)
viewNodeRemoteLines nodes status =
    let
        sourceNode =
            findNode status.nodeId nodes
    in
    case sourceNode of
        Just source ->
            status.remotes
                |> List.filterMap (viewRemoteLine nodes source)

        Nothing ->
            []


viewRemoteLine : List Node -> Node -> RemoteInfo -> Maybe (Svg msg)
viewRemoteLine nodes sourceNode remote =
    case remote.targetNodeId of
        Just targetId ->
            case findNode targetId nodes of
                Just targetNode ->
                    Just
                        (line
                            [ SA.x1 (String.fromFloat (sourceNode.positionX + 100))
                            , SA.y1 (String.fromFloat (sourceNode.positionY + 50))
                            , SA.x2 (String.fromFloat (targetNode.positionX + 100))
                            , SA.y2 (String.fromFloat (targetNode.positionY + 50))
                            , SA.stroke "#2196f3"
                            , SA.strokeWidth "2"
                            , SA.strokeDasharray "5,5"
                            , SA.markerEnd "url(#arrowhead)"
                            ]
                            []
                        )

                Nothing ->
                    Nothing

        Nothing ->
            Nothing


findNode : String -> List Node -> Maybe Node
findNode nodeId nodes =
    nodes
        |> List.filter (\n -> n.id == nodeId)
        |> List.head


-- EVENT HANDLERS


onMouseMove : (Position -> msg) -> Svg.Attribute msg
onMouseMove toMsg =
    Html.Events.on "mousemove"
        (Decode.map toMsg positionDecoder)


onMouseUp : msg -> Svg.Attribute msg
onMouseUp msg =
    Html.Events.on "mouseup" (Decode.succeed msg)


onWheel : (Float -> msg) -> Svg.Attribute msg
onWheel toMsg =
    Html.Events.preventDefaultOn "wheel"
        (Decode.map (\delta -> ( toMsg delta, True ))
            (Decode.field "deltaY" Decode.float)
        )


onNodeMouseDown : String -> (String -> Position -> msg) -> Svg.Attribute msg
onNodeMouseDown nodeId toMsg =
    Html.Events.stopPropagationOn "mousedown"
        (Decode.map (\pos -> ( toMsg nodeId pos, True ))
            positionDecoder
        )


preventContextMenu : (Position -> msg) -> Svg.Attribute msg
preventContextMenu toMsg =
    Html.Events.preventDefaultOn "contextmenu"
        (Decode.map (\pos -> ( toMsg pos, True ))
            positionDecoder
        )


positionDecoder : Decode.Decoder Position
positionDecoder =
    Decode.map2 Position
        (Decode.field "clientX" Decode.float)
        (Decode.field "clientY" Decode.float)
