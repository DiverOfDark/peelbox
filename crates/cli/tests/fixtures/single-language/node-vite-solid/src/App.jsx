import { createSignal } from 'solid-js';

function App() {
  const [count, setCount] = createSignal(0);
  return (
    <main>
      <h1>Vite + Solid</h1>
      <button onClick={() => setCount(count() + 1)}>count is {count()}</button>
    </main>
  );
}

export default App;
