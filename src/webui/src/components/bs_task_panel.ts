import {LitElement, html} from 'lit';
import {customElement, property} from 'lit/decorators.js';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import Handlebars from 'handlebars';
import templateContent from './bs_task_panel.template?raw';
import '@shoelace-style/shoelace/dist/components/progress-bar/progress-bar.js';

@customElement('bs-task-panel')
class BSTaskPanel extends LitElement {
    static properties = {
        title: { type: String },
        eta: { type: String },
        progress: { type: Number },
    };

    template_compiled: HandlebarsTemplateDelegate<any>;

    constructor() {
        super();
        this.template_compiled = Handlebars.compile(templateContent);
    }

    render() {
        let uidata = {
            title: this.title,
            eta: this.eta,
            progress: this.progress
        };
        let real_content = this.template_compiled(uidata);
        //console.log(real_content);
        return html`${unsafeHTML(real_content)}`;
    }
  }
  
