import { useState } from 'preact/hooks';

export function App() {
  const [count, setCount] = useState(0);
  return (
    <main>
      <h1>Vite + Preact</h1>
      <button onClick={() => setCount(count + 1)}>count is {count}</button>
    </main>
  );
}
