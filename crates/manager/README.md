# TerminusDB Manager

Web-based management interface for multiple TerminusDB instances.

## Overview

TerminusDB Manager provides:
- Visual canvas for managing multiple TerminusDB instances
- Drag-and-drop node positioning
- Real-time health monitoring
- Automatic remote connection visualization
- Built-in local TerminusDB instance

## Architecture

- **Backend**: Rust with Rocket web framework
- **Frontend**: Elm with SVG canvas
- **Local Instance**: Embedded TerminusDB binary (via `terminusdb-bin`)
- **Static Assets**: Embedded with `rust-embed` for single-binary deployment

## Building

### Prerequisites

- Rust (nightly, for Rocket)
- Elm 0.19.1 (for frontend development)
- Dependencies for terminusdb-bin (SWI-Prolog, protoc, GMP)

### Development Build

1. **Compile Elm frontend** (optional for development):
```bash
cd frontend
elm make src/Main.elm --output=public/app.js
```

2. **Run the server**:
```bash
cargo run --release
```

The server will:
- Start a local TerminusDB instance on port 6363
- Serve the web interface on port 8000
- Poll all configured nodes every 5 seconds

### Production Build

For production, compile Elm with optimizations:

```bash
cd frontend
elm make src/Main.elm --optimize --output=public/app.js
```

Optionally minify:

```bash
uglifyjs public/app.js --compress 'pure_funcs=[F2,F3,F4,F5,F6,F7,F8,F9,A2,A3,A4,A5,A6,A7,A8,A9],pure_getters,keep_fargs=false,unsafe_comps,unsafe' | uglifyjs --mangle --output public/app.min.js
mv public/app.min.js public/app.js
```

Then build the Rust binary:

```bash
cargo build --release
```

The resulting binary embeds all frontend assets and the TerminusDB binary.

## Usage

### Starting the Manager

```bash
./target/release/terminusdb-manager
```

Access the web interface at http://localhost:8000

### Configuration

Set environment variables:
- `TERMINUSDB_ADMIN_PASS` or `TERMINUSDB_PASS` - Password for local instance (default: "root")
- `ROCKET_PORT` - Web server port (default: 8000)
- `ROCKET_ADDRESS` - Bind address (default: 127.0.0.1)

### Adding Nodes

1. Right-click on the canvas
2. Select "Add Node"
3. Fill in the form:
   - **Label**: Display name
   - **Host**: Hostname or IP
   - **Port**: TerminusDB port (usually 6363)
   - **Username**: Admin username
   - **Password**: Admin password
   - **SSH Enabled**: For future bundle transfer feature

### Managing Nodes

- **Move**: Click and drag nodes
- **View Status**: Green (online), Red (offline), Gray (unknown)
- **Database Count**: Displayed on each node
- **Remote Connections**: Blue dashed lines to remote instances

## API Endpoints

- `GET /api/nodes` - List all nodes
- `POST /api/nodes` - Create node
- `PUT /api/nodes/:id` - Update node
- `DELETE /api/nodes/:id` - Delete node
- `GET /api/status` - Get all statuses
- `GET /api/instance/local` - Local instance info

## Development

See [frontend/README.md](frontend/README.md) for Elm development workflow.

## Testing

Run with Playwright (TODO):

```bash
npx playwright test
```

## TODO

- [ ] Persist nodes in local TerminusDB instance (currently in-memory)
- [ ] SSH bundle transfer for slow/unreliable networks
- [ ] Node search/filter
- [ ] Export/import configurations
- [ ] Multi-user support with authentication
