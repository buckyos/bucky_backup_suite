export class PanelList extends HTMLElement {
  title: string = '';
  showAddButton: boolean = true;
  
  constructor() {
    super();
  }

  connectedCallback() {

    const template = document.createElement('template');
    template.innerHTML = `
    <style>
      .panel-list-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
      }
    </style>
    <div class="panel-list">
      <div class="panel-list-header">
        <div id="title"></div>
        <sl-icon-button id="add-button" name="plus-circle" label="Add"></sl-icon-button>
      </div>
      <div id="panel-content">
        <slot></slot>
      </div>
    </div>
    `;
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.appendChild(template.content.cloneNode(true));

    this.title = this.getAttribute('title') || '';
    this.showAddButton = true;

    // 添加按钮点击事件
    if (this.showAddButton) {
      this.shadowRoot.querySelector('#add-button').addEventListener('click', () => {
        this.dispatchEvent(new CustomEvent('add-click'));
      });
    }

    this.shadowRoot.querySelector('#title').textContent = this.title;
  }

  attributeChangedCallback(attributeName:string, oldValue:string, newValue:string) {
    if (attributeName === 'title') {
      this.title = newValue;
      this.shadowRoot.querySelector('#title').textContent = this.title;
    }

    if (attributeName === 'add-button') {
      this.showAddButton = newValue === 'true';
      if (this.showAddButton) {
        this.shadowRoot.querySelector('#add-button').textContent = '+';
      } else {
        this.shadowRoot.querySelector('#add-button').textContent = '';
      }
    }
  }
}

customElements.define("panel-list", PanelList);
