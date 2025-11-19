# TerminusDB Manager Frontend

Elm-based frontend for managing multiple TerminusDB instances.

## Development

### Prerequisites

- [Elm 0.19.1](https://guide.elm-lang.org/install/elm.html)
- Node.js (for elm-live)

### Install Dependencies

```bash
npm install -g elm elm-live
```

### Development Server

Run with live reload:

```bash
elm-live src/Main.elm --open -- --output=public/app.js
```

Or use the provided script:

```bash
./dev.sh
```

The frontend will be available at http://localhost:8000

### Production Build

Compile and optimize:

```bash
elm make src/Main.elm --optimize --output=public/app.js
```

For further optimization:

```bash
uglifyjs public/app.js --compress 'pure_funcs=[F2,F3,F4,F5,F6,F7,F8,F9,A2,A3,A4,A5,A6,A7,A8,A9],pure_getters,keep_fargs=false,unsafe_comps,unsafe' | uglifyjs --mangle --output public/app.min.js
```

## Features

### Canvas

- **Pan**: Click and drag on empty space
- **Zoom**: Mouse wheel
- **Add Node**: Right-click on canvas
- **Move Node**: Click and drag node

### Node Display

- **Status Indicator**: Green (online) / Red (offline) / Gray (unknown)
- **Database Count**: Shows number of databases on instance
- **Remote Connections**: Lines drawn to remote instances

### Node Management

- Create new nodes via right-click menu
- Edit node configuration (TODO)
- Delete nodes (TODO)
- Real-time status updates every 2 seconds

## Architecture

- `Main.elm` - Application entry point, model, update, subscriptions
- `Types.elm` - All type definitions
- `Api.elm` - HTTP client for backend API
- `Canvas.elm` - SVG canvas rendering and interactions

## TODO

- [ ] Add node editing functionality
- [ ] Add node deletion confirmation
- [ ] Implement SSH features (stubbed)
- [ ] Add search/filter for nodes
- [ ] Export/import node configurations
- [ ] Keyboard shortcuts
