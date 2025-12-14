#!/bin/bash
# Development server with live reload

echo "Starting Elm development server..."
echo "Backend API should be running at http://localhost:8000/api"
echo "Note: You may need to configure CORS if running backend separately"
echo ""

elm-live src/Main.elm --open -- --output=public/app.js
