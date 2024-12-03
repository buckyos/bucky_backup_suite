import {LitElement, html} from 'lit';
import {customElement, property} from 'lit/decorators.js';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import Handlebars from 'handlebars';
import { TaskInfo } from '../utils/task_mgr';
import { SlProgressBar } from "@shoelace-style/shoelace";
import templateContent from './bs_task_panel.template?raw';

@customElement('bs-task-panel')
export class BSTaskPanel extends LitElement {
    static properties = {
        task_title: { type: String },
        eta: { type: String },
        complete_item: { type: Number },
        total_item: { type: Number },
        completed_size: { type: String },
        total_size: { type: String },
        last_log_content: { type: String },
        task_state: { type: String },
        progress_value: { type: Number },
        progress_text: { type: String },
    };

    task_state: string;
    total_item: number;
    complete_item: number;
    completed_size: string;
    total_size: string;
    last_log_content: string | null;
    last_update_task_info: TaskInfo | null;
    last_update_time: number;
    task_title: string;
    eta: string;
    progress_value: number;
    progress_text: string;

    template_compiled: HandlebarsTemplateDelegate<any>;

    updateTaskInfo(task_info: TaskInfo) {
        this.task_title = task_info.taskid;//换成owner plan的title

        if (task_info.total_size != 0 ) {
            this.progress_value = (task_info.completed_size * 100) / task_info.total_size;
            this.progress_text = `${((task_info.completed_size * 100) / task_info.total_size).toFixed(1)}%`;
        }
        switch(task_info.state) {
            case 'RUNNING':
                this.task_state = "upload";
                break;
            case 'PENDING':
                this.task_state = "hourglass";
                break;
            case 'PAUSED':
                this.task_state = "pause-fill";
                break;
            case 'DONE':
                this.task_state = "check-square";
                break;
            case 'FAILED':
                this.task_state = "x-circle";
                break;
        }
        let now = Date.now();
        if(this.last_update_task_info) {
            // Calculate speed and ETA
            let delta_time = (now - this.last_update_time) / 1000; // Convert to seconds
            let delta_size = task_info.completed_size - this.last_update_task_info.completed_size;
            
            // Avoid division by zero
            if (delta_time > 0) {
                let speed = delta_size / delta_time; // bytes per second
                let speedStr = '';
                
                // Convert speed to appropriate unit
                if (speed > 1024 * 1024) {
                    speedStr = `${(speed / (1024 * 1024)).toFixed(2)} MB/s`;
                } else if (speed > 1024) {
                    speedStr = `${(speed / 1024).toFixed(2)} KB/s`;
                } else {
                    speedStr = `${Math.round(speed)} B/s`;
                }

                // Calculate ETA
                let remaining_size = task_info.total_size - task_info.completed_size;
                let eta_seconds = remaining_size / speed;
                
                // Format ETA
                let eta_str = '';
                if (eta_seconds > 3600) {
                    eta_str = `${Math.floor(eta_seconds / 3600)}h ${Math.floor((eta_seconds % 3600) / 60)}m`;
                } else if (eta_seconds > 60) {
                    eta_str = `${Math.floor(eta_seconds / 60)}m ${Math.floor(eta_seconds % 60)}s`;
                } else {
                    eta_str = `${Math.floor(eta_seconds)}s`;
                }

                this.eta = `${speedStr}, ETA: ${eta_str}`;
            }
        }
        this.complete_item = task_info.completed_item_count;
        this.total_item = task_info.item_count;
        this.completed_size = task_info.completed_size.toString();
        this.total_size = task_info.total_size.toString();
        this.last_update_time = now;
        this.last_update_task_info = task_info;
        this.requestUpdate();
    }

    constructor() {
        super();
        this.eta = "";
        this.complete_item = 0;
        this.total_item = 0;
        this.completed_size = "0";
        this.total_size = "Unknown";
        this.last_update_time = 0;
        this.last_update_task_info = null;
        this.task_title = "Test Task";
        this.last_log_content = "Starting...";
        this.task_state = "pause-fill";
        this.progress_value = 0;
        this.progress_text = "0%";
        this.template_compiled = Handlebars.compile(templateContent);
    }

    render() {
        return html`
<style>
    .task-panel {
      border: 1px solid #ccc;
      border-radius: 8px;
      padding: 15px;
      margin: 10px;
      font-family: Arial, sans-serif;
    }

    .header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 10px;
    }

    .title {
      font-size: 18px;
      font-weight: bold;
    }

    .eta {
      color: #666;
    }

    .status {
      display: flex;
      justify-content: space-between;
      margin-top: 8px;
      color: #666; 
    }
  </style>

<div class="task-panel">
    <div class="header">
      <sl-icon-button id="task-state" name="${this.task_state}"></sl-icon-button>
      <div class="title">${this.task_title}</div>
      <div class="eta">${this.eta}</div>
    </div>
    <sl-progress-bar id="work-progress-bar" value="${this.progress_value}">${this.progress_text}</sl-progress-bar>
    <div class="status">
      <div class="upload-status">${this.last_log_content}</div>
      <div class="size-info">(${this.complete_item}/${this.total_item}),${this.completed_size} / ${this.total_size}</div>
    </div>
</div>

        `;
    }
}

