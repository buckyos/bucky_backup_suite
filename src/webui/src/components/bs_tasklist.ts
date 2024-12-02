import {LitElement, html} from 'lit';
import {customElement, property} from 'lit/decorators.js';


@customElement('bs-tasklist')
class BSTaskList extends LitElement {
    constructor() {
        super();
    }

    render() {
        return html`<div>
            <div class="task-list-container">
                <bs-task-panel title="Test Task" eta="3 hours" progress="58"></bs-task-panel>
            </div>
        `;
    }
  }