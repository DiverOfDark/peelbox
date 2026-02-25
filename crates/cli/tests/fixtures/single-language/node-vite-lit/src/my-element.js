import { LitElement, html, css } from 'lit';

export class MyElement extends LitElement {
  static properties = {
    count: { type: Number },
  };

  static styles = css`
    :host { display: block; padding: 16px; }
  `;

  constructor() {
    super();
    this.count = 0;
  }

  render() {
    return html`
      <h1>Vite + Lit</h1>
      <button @click=${() => this.count++}>count is ${this.count}</button>
    `;
  }
}

customElements.define('my-element', MyElement);
