import {LitElement, html} from 'lit';
import {customElement, property} from 'lit/decorators.js';
import { SlInput, SlSelect, SlCheckbox } from '@shoelace-style/shoelace';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import Handlebars from 'handlebars';
import templateContent from './bs_s3_config.template?raw';

@customElement('bs-s3-config')
export class BSS3Config extends LitElement {
    @property({ type: String }) bucket: string = '';
    @property({ type: String }) region: string = '';
    @property({ type: String }) prefix: string = '';
    @property({ type: Boolean }) use_env_credentials: boolean = true;
    @property({ type: String }) access_key_id: string = '';
    @property({ type: String }) secret_access_key: string = '';
    @property({ type: String }) session_token: string = '';

    private template_compiled: HandlebarsTemplateDelegate;
    
    private regions = [
        { value: 'us-east-1', label: 'US East (N. Virginia)' },
        { value: 'us-east-2', label: 'US East (Ohio)' },
        { value: 'us-west-1', label: 'US West (N. California)' },
        { value: 'us-west-2', label: 'US West (Oregon)' },
        { value: 'ap-east-1', label: 'Asia Pacific (Hong Kong)' },
        { value: 'ap-south-1', label: 'Asia Pacific (Mumbai)' },
        { value: 'ap-northeast-1', label: 'Asia Pacific (Tokyo)' },
        { value: 'ap-northeast-2', label: 'Asia Pacific (Seoul)' },
        { value: 'ap-southeast-1', label: 'Asia Pacific (Singapore)' },
        { value: 'ap-southeast-2', label: 'Asia Pacific (Sydney)' },
        { value: 'eu-central-1', label: 'Europe (Frankfurt)' },
        { value: 'eu-west-1', label: 'Europe (Ireland)' },
        { value: 'eu-west-2', label: 'Europe (London)' },
        { value: 'eu-west-3', label: 'Europe (Paris)' },
        { value: 'eu-north-1', label: 'Europe (Stockholm)' }
    ];

    constructor() {
        super();
        this.template_compiled = Handlebars.compile(templateContent);
    }

    firstUpdated() {
        const useEnvCheckbox = this.shadowRoot?.querySelector('#use-env-credentials') as SlCheckbox;
        if (useEnvCheckbox) {
            useEnvCheckbox.addEventListener('sl-change', () => {
                this.use_env_credentials = useEnvCheckbox.checked;
                this.requestUpdate();
            });
        }

        // Add event listeners for inputs
        const bucketInput = this.shadowRoot?.querySelector('#bucket') as SlInput;
        if (bucketInput) {
            bucketInput.addEventListener('sl-change', (e: any) => {
                this.bucket = e.target.value;
            });
        }

        const regionSelect = this.shadowRoot?.querySelector('#region') as SlSelect;
        if (regionSelect) {
            regionSelect.addEventListener('sl-change', (e: any) => {
                this.region = e.target.value;
            });
        }

        const prefixInput = this.shadowRoot?.querySelector('#prefix') as SlInput;
        if (prefixInput) {
            prefixInput.addEventListener('sl-change', (e: any) => {
                this.prefix = e.target.value;
            });
        }

        const accessKeyInput = this.shadowRoot?.querySelector('#access-key') as SlInput;
        if (accessKeyInput) {
            accessKeyInput.addEventListener('sl-change', (e: any) => {
                this.access_key_id = e.target.value;
            });
        }

        const secretKeyInput = this.shadowRoot?.querySelector('#secret-key') as SlInput;
        if (secretKeyInput) {
            secretKeyInput.addEventListener('sl-change', (e: any) => {
                this.secret_access_key = e.target.value;
            });
        }

        const sessionTokenInput = this.shadowRoot?.querySelector('#session-token') as SlInput;
        if (sessionTokenInput) {
            sessionTokenInput.addEventListener('sl-change', (e: any) => {
                this.session_token = e.target.value;
            });
        }
    }

    getConfig() {
        const config = {
            bucket: this.bucket,
            region: this.region,
            prefix: this.prefix,
            session: this.use_env_credentials ? 
                { type: 'env' } :
                {
                    type: 'key',
                    access_key_id: this.access_key_id,
                    secret_access_key: this.secret_access_key,
                    session_token: this.session_token || undefined
                }
        };
        return config;
    }

    setConfig(config: any) {
        this.bucket = config.bucket || '';
        this.region = config.region || '';
        this.prefix = config.prefix || '';
        
        if (config.session) {
            this.use_env_credentials = config.session.type === 'env';
            if (config.session.type === 'key') {
                this.access_key_id = config.session.access_key_id || '';
                this.secret_access_key = config.session.secret_access_key || '';
                this.session_token = config.session.session_token || '';
            }
        }
        
        this.requestUpdate();
    }

    render() {
        const templateData = {
            bucket: this.bucket,
            region: this.region,
            prefix: this.prefix,
            use_env_credentials: this.use_env_credentials,
            access_key_id: this.access_key_id,
            secret_access_key: this.secret_access_key,
            session_token: this.session_token,
            regions: this.regions
        };

        return html`${unsafeHTML(this.template_compiled(templateData))}`;
    }
} 