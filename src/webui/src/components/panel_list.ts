import { taskManager } from "../utils/task_mgr";

export class PanelList extends HTMLElement {
  title: string = '';
  showAddButton: boolean = true;
  
  constructor() {
    super();
  }

  add_panel(panel:HTMLElement,panel_id:string) {
    let panel_content = this.shadowRoot?.querySelector('#panel-content');
    if(panel_content) {
      panel_content.appendChild(panel);
    }
  }

  remove_panel(panel_id:string) {
    let panel_content = this.shadowRoot?.querySelector('#panel-content');
    if(panel_content) {
      let panel = panel_content.querySelector(`#${panel_id}`);
      if(panel) {
        panel_content.removeChild(panel);
      }
    }
  }

  clear_panels() {
    let panel_content = this.shadowRoot?.querySelector('#panel-content');
    if(panel_content) {
      panel_content.innerHTML = '';
    }
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
      this.shadowRoot?.querySelector('#add-button')?.addEventListener('click', () => {
        this.dispatchEvent(new CustomEvent('add-click'));
      });
    }

    let title_element = this.shadowRoot?.querySelector('#title');
    if(title_element) {
      title_element.textContent = this.title;
    }
  }

  attributeChangedCallback(attributeName:string, oldValue:string, newValue:string) {
    if (attributeName === 'title') {
      this.title = newValue;
      let title_element = this.shadowRoot?.querySelector('#title');
      if(title_element) {
        title_element.textContent = this.title;
      }
    }

    if (attributeName === 'add-button') {
      this.showAddButton = newValue === 'true';
      if (this.showAddButton) {
        let add_button = this.shadowRoot?.querySelector('#add-button');
        if(add_button) {
          add_button.textContent = '+';
        }
      } else {
        let add_button = this.shadowRoot?.querySelector('#add-button');
        if(add_button) {
          add_button.textContent = '';
        }
      }
    }
  }
}

customElements.define("panel-list", PanelList);

