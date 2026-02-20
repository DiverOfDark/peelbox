import React from 'react';
import { createRoot } from 'react-dom/client';

function App() {
    return (
        <div>
            <h1>Laravel React App</h1>
        </div>
    );
}

const root = createRoot(document.getElementById('app'));
root.render(<App />);
