import {LitElement, html, css} from 'lit';
import {customElement, property} from 'lit/decorators.js';
import { SlInput, SlSelect, SlCheckbox } from '@shoelace-style/shoelace';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import Handlebars from 'handlebars';
import templateContent from './bs_s3_config.template?raw';

@customElement('bs-s3-config')
export class BSS3Config extends LitElement {
    static styles = css`
        :host {
            display: block;
            width: 100%;
            min-height: 100px;
            border: 1px solid transparent;
        }
    `;

    @property({ type: String })
    get bucket() { return this._bucket; }
    set bucket(value: string) { 
        const oldValue = this._bucket;
        this._bucket = value;
        this.requestUpdate('bucket', oldValue);
    }
    private _bucket: string = '';

    @property({ type: String })
    get region() { return this._region; }
    set region(value: string) {
        const oldValue = this._region;
        this._region = value;
        this.requestUpdate('region', oldValue);
    }
    private _region: string = 'us-east-1';

    @property({ type: Boolean })
    get use_env_credentials() { return this._use_env_credentials; }
    set use_env_credentials(value: boolean) {
        const oldValue = this._use_env_credentials;
        this._use_env_credentials = value;
        this.requestUpdate('use_env_credentials', oldValue);
    }
    private _use_env_credentials: boolean = true;

    @property({ type: String })
    get access_key_id() { return this._access_key_id; }
    set access_key_id(value: string) {
        const oldValue = this._access_key_id;
        this._access_key_id = value;
        this.requestUpdate('access_key_id', oldValue);
    }
    private _access_key_id: string = '';

    @property({ type: String })
    get secret_access_key() { return this._secret_access_key; }
    set secret_access_key(value: string) {
        const oldValue = this._secret_access_key;
        this._secret_access_key = value;
        this.requestUpdate('secret_access_key', oldValue);
    }
    private _secret_access_key: string = '';

    @property({ type: String })
    get session_token() { return this._session_token; }
    set session_token(value: string) {
        const oldValue = this._session_token;
        this._session_token = value;
        this.requestUpdate('session_token', oldValue);
    }
    private _session_token: string = '';

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
        try {
            this.template_compiled = Handlebars.compile(templateContent);
        } catch (error) {
            console.error('Template compilation failed:', error);
        }
        this.region = 'us-east-1';
    }

    createRenderRoot() {
        return this;
    }

    connectedCallback() {
        super.connectedCallback();
        this.requestUpdate();
    }

    firstUpdated() {
        setTimeout(() => {
            this.bindEvents();
        }, 0);
    }

    private bindEvents() {
        const useEnvCheckbox = this.querySelector('#use-env-credentials') as SlCheckbox;
        if (useEnvCheckbox) {
            useEnvCheckbox.addEventListener('sl-change', () => {
                this.use_env_credentials = useEnvCheckbox.checked;
                this.requestUpdate();
            });
        }

        const bucketInput = this.querySelector('#bucket') as SlInput;
        if (bucketInput) {
            bucketInput.addEventListener('sl-change', (e: any) => {
                this.bucket = e.target.value;
            });
        }

        const regionSelect = this.querySelector('#region') as SlSelect;
        if (regionSelect) {
            regionSelect.addEventListener('sl-change', (e: any) => {
                this.region = e.target.value;
            });
        }

        const accessKeyInput = this.querySelector('#access-key') as SlInput;
        if (accessKeyInput) {
            accessKeyInput.addEventListener('sl-change', (e: any) => {
                this.access_key_id = e.target.value;
            });
        }

        const secretKeyInput = this.querySelector('#secret-key') as SlInput;
        if (secretKeyInput) {
            secretKeyInput.addEventListener('sl-change', (e: any) => {
                this.secret_access_key = e.target.value;
            });
        }

        const sessionTokenInput = this.querySelector('#session-token') as SlInput;
        if (sessionTokenInput) {
            sessionTokenInput.addEventListener('sl-change', (e: any) => {
                this.session_token = e.target.value;
            });
        }
    }

    getUrl() {
        let url = `s3://${this.bucket}/${this.region}`;
        
        const params = new URLSearchParams();
        
        if (!this.use_env_credentials) {
            params.set('type', 'key');
            params.set('access_key', this.access_key_id);
            params.set('secret_key', this.secret_access_key);
        }
        
        const copy_id = Date.now().toString();
        params.set('copy_id', copy_id);
        
        const queryString = params.toString();
        if (queryString) {
            url += `?${queryString}`;
        }
        
        return url;
    }

    render() {
        try {
            const templateData = {
                bucket: this.bucket || '',
                region: this.region || 'us-east-1',
                use_env_credentials: this.use_env_credentials,
                access_key_id: this.access_key_id || '',
                secret_access_key: this.secret_access_key || '',
                session_token: this.session_token || '',
                regions: this.regions
            };

            const rendered = this.template_compiled(templateData);
            return html`<div>${unsafeHTML(rendered)}</div>`;
        } catch (error) {
            console.error('Render failed:', error);
            return html`<div>Error rendering component: ${error.message}</div>`;
        }
    }
} 