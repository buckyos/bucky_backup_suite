import templateContent from './wizzard_dlg.template?raw';
import { SlDialog } from '@shoelace-style/shoelace';

//该组件，可以往里push dlg(另一个component).当有dlg时，左上角有back按钮。每次push时，当前的dlg会往左淡出，新的dlg从右边进场。 

export class BuckyWizzardDlg extends HTMLElement {
    private dlgStack: {dlg:HTMLElement, title:string}[] = [];
    public wizzard_data: any = {};
    
    constructor() {
      super();
      
    }

    connectedCallback() {
      const template = document.createElement('template');
      template.innerHTML = templateContent;
      const shadow = this.attachShadow({ mode: 'open' });
      shadow.appendChild(template.content.cloneNode(true));
      const init_title = this.getAttribute("title") || "";

      const backButton = this.shadowRoot?.getElementById('back-button');
      if (backButton) {
        backButton.addEventListener('click', () => { 
          this.popDlg()
        });
      }

      const slot = this.shadowRoot?.getElementById('dlg-content') as HTMLSlotElement | null;
      if (!slot) {
        return;
      }

      const closeButton = this.shadowRoot?.getElementById('close-button');
      if (closeButton) {
        closeButton.addEventListener('click', () => {
          let parent = this.parentElement as SlDialog;
          if (parent) {
            parent.hide();
          }
        });
      }

      const dlgContent = slot.assignedElements() as HTMLElement[];
      this.setTitle(init_title);
      this.dlgStack.push({dlg:dlgContent[0], title:init_title});      
    }
    
    setTitle(title: string) {
      if (!this.shadowRoot)
        return;

      const title_element = this.shadowRoot?.getElementById('dlg-title');
      if (title_element) {
        title_element.textContent = title;
      }
    }

    getTitle() {
      if (!this.shadowRoot)
        return "";

      const title_element = this.shadowRoot?.getElementById('dlg-title');
      return title_element?.textContent || "";
    }

    pushDlg(dlg: HTMLElement, title: string) {
      if (!this.shadowRoot)
        return;

      const container = this.shadowRoot.querySelector('#dlg-frame') as HTMLElement;

      if (this.dlgStack.length > 0) {
        const current_slot = this.dlgStack[this.dlgStack.length - 1];
        if (current_slot) {
          current_slot.dlg.style.display = 'none';
        }
      }

      const title_element = this.shadowRoot?.getElementById('dlg-title');
      if (title_element) {
        title_element.textContent = title;
      }

      this.dlgStack.push({dlg:dlg, title:title});
      container.appendChild(dlg);
      this.updateBackButton();
    }

    popDlg() {
      if (this.dlgStack.length <= 1) 
        return;

      const current_slot = this.dlgStack.pop();
      if (current_slot) {
        current_slot.dlg.remove();
      }

      const previous_slot = this.dlgStack[this.dlgStack.length - 1];
      if (previous_slot) {
        previous_slot.dlg.style.display = 'block';
      }
      this.setTitle(previous_slot.title);
      this.updateBackButton();
    }

    disableBackButton() {
      if (!this.shadowRoot)
        return;

      const backButton = this.shadowRoot.querySelector('#back-button') as HTMLButtonElement;
      backButton.style.display = 'none';
    }

    private updateBackButton() {
      if (!this.shadowRoot) 
        return;

      const backButton = this.shadowRoot.querySelector('#back-button') as HTMLButtonElement;
      backButton.style.display = this.dlgStack.length > 1 ? 'block' : 'none';
    }

    closeDlg() {
      let parent = this.parentElement as SlDialog;
      if (parent) {
        parent.hide();
      }
    }
}

customElements.define('bucky-wizzard-dlg', BuckyWizzardDlg);


