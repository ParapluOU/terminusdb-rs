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
    , onWheel : Float -> Float -> msg
    , onContextMenu : Position -> msg
    , onNodeContextMenu : String -> Position -> msg
    , onNodeClick : String -> Position -> msg
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
                [ -- Drop shadow for nodes (using feGaussianBlur + feOffset)
                  Svg.filter [ SA.id "shadow" ]
                    [ Svg.feGaussianBlur
                        [ SA.in_ "SourceAlpha"
                        , SA.stdDeviation "2"
                        ]
                        []
                    , Svg.feOffset
                        [ SA.dx "0"
                        , SA.dy "2"
                        , SA.result "offsetblur"
                        ]
                        []
                    , Svg.feComponentTransfer []
                        [ Svg.feFuncA [ SA.type_ "linear", SA.slope "0.2" ] [] ]
                    , Svg.feMerge []
                        [ Svg.feMergeNode [] []
                        , Svg.feMergeNode [ SA.in_ "SourceGraphic" ] []
                        ]
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

            , -- Transform group for pan/zoom
              g
                [ SA.transform
                    (transformString config.canvasView)
                ]
                [ -- Background grid
                  viewGrid config.canvasView

                , -- Remote connection lines
                  g [] (viewRemoteLines config.nodes config.statuses)

                , -- Nodes
                  g [] (List.map (viewNode config.statuses config.onMouseDown config.onNodeContextMenu config.onNodeClick) config.nodes)
                ]
            ]
        ]


transformString : CanvasView -> String
transformString canvasView =
    "translate(" ++ String.fromFloat canvasView.offsetX ++ "," ++ String.fromFloat canvasView.offsetY ++ ") scale(" ++ String.fromFloat canvasView.zoom ++ ")"


-- GRID


viewGrid : CanvasView -> Svg msg
viewGrid canvasView =
    let
        gridSize =
            50

        gridOpacity =
            "0.4"

        dotRadius =
            "2"

        -- Generate grid dots at intersection points
        gridDots =
            List.range -30 30
                |> List.concatMap
                    (\x ->
                        List.range -30 30
                            |> List.map
                                (\y ->
                                    Svg.circle
                                        [ SA.cx (String.fromFloat (toFloat x * gridSize))
                                        , SA.cy (String.fromFloat (toFloat y * gridSize))
                                        , SA.r dotRadius
                                        , SA.fill "#666"
                                        , SA.opacity gridOpacity
                                        ]
                                        []
                                )
                    )
    in
    g [] gridDots


-- NODES


viewNode : List NodeStatus -> (String -> Position -> msg) -> (String -> Position -> msg) -> (String -> Position -> msg) -> Node -> Svg msg
viewNode statuses onMouseDown onNodeContextMenu onNodeClick node =
    let
        status =
            findStatus node.id statuses

        ( fillColor, strokeColor ) =
            case status of
                Just s ->
                    case s.connectivity of
                        Accessible ->
                            ( "#ffffff", "#4caf50" )  -- Green

                        Reachable ->
                            ( "#ffffff", "#ffa726" )  -- Yellow/Orange

                        Unreachable ->
                            ( "#ffffff", "#f44336" )  -- Red

                Nothing ->
                    ( "#ffffff", "#999999" )  -- Gray

        databaseCount =
            status
                |> Maybe.map .databaseCount
                |> Maybe.withDefault 0

        remoteCount =
            status
                |> Maybe.map (\s -> List.length s.remotes)
                |> Maybe.withDefault 0
    in
    g
        [ SA.transform ("translate(" ++ String.fromFloat node.positionX ++ "," ++ String.fromFloat node.positionY ++ ")")
        , SA.class "node"
        , SA.cursor "move"
        , SA.style "user-select: none; -webkit-user-select: none; -moz-user-select: none;"
        , onNodeMouseDown node.id onMouseDown
        , onNodeRightClick node.id onNodeContextMenu
        , onNodeClickHandler node.id onNodeClick
        ]
        [ -- Node background
          rect
            [ SA.width "200"
            , SA.height "120"
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
            [ text (node.host ++ ":" ++ String.fromInt node.portNumber) ]

        , -- Database count
          text_
            [ SA.x "20"
            , SA.y "70"
            , SA.fontSize "12"
            , SA.fill "#666"
            ]
            [ text ("📊 " ++ String.fromInt databaseCount ++ " database" ++ (if databaseCount == 1 then "" else "s")) ]

        , -- Remote count
          text_
            [ SA.x "20"
            , SA.y "90"
            , SA.fontSize "12"
            , SA.fill "#666"
            ]
            [ text ("🔗 " ++ String.fromInt remoteCount ++ " remote" ++ (if remoteCount == 1 then "" else "s")) ]
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


onWheel : (Float -> Float -> msg) -> Svg.Attribute msg
onWheel toMsg =
    Html.Events.preventDefaultOn "wheel"
        (Decode.map2 (\deltaX deltaY -> ( toMsg deltaX deltaY, True ))
            (Decode.field "deltaX" Decode.float)
            (Decode.field "deltaY" Decode.float)
        )


onNodeMouseDown : String -> (String -> Position -> msg) -> Svg.Attribute msg
onNodeMouseDown nodeId toMsg =
    Html.Events.stopPropagationOn "mousedown"
        (Decode.map (\pos -> ( toMsg nodeId pos, True ))
            positionDecoder
        )


onNodeRightClick : String -> (String -> Position -> msg) -> Svg.Attribute msg
onNodeRightClick nodeId toMsg =
    Html.Events.preventDefaultOn "contextmenu"
        (Decode.map (\pos -> ( toMsg nodeId pos, True ))
            positionDecoder
        )


onNodeClickHandler : String -> (String -> Position -> msg) -> Svg.Attribute msg
onNodeClickHandler nodeId toMsg =
    Html.Events.stopPropagationOn "click"
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
