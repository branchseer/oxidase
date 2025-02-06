<script lang="ts">
  import CodeMirror from "svelte-codemirror-editor";
  import { javascript } from "@codemirror/lang-javascript";
  import { EditorView } from "codemirror";
  import { transpile } from "oxidase";
  import exampleSource from './example?raw'

  const jsLang = javascript();
  const tsLang = javascript({ typescript: true });
  const theme = EditorView.theme({
    "&.cm-editor": { height: "100%" },
  });

  let source = $state(exampleSource);

  let output = $derived.by(() => {
    try {
      return {
        text: transpile("playground-input.ts", source),
        isError: false,
      };
    } catch (err) {
      return {
        text: `${err}`,
        isError: true,
      };
    }
  });
</script>

<main>
  <header>
    <h1>Oxidase</h1>
    <a href="https://github.com/branchseer/oxidase">GitHub</a>
  </header>
  <div class="content">
    <div class="editor-container">
      <CodeMirror
        class="editor"
        {theme}
        nodebounce
        bind:value={source}
        lang={tsLang}
      />
    </div>
    <div class="divider"></div>
    <div class="editor-container">
      <CodeMirror
        class="editor"
        readonly
        {theme}
        value={output.text}
        lang={output.isError ? null : jsLang}
      />
    </div>
  </div>
</main>

<style>
  header {
     padding-inline-start: 26px;
  }
  h1 {
    font-size: 20px;
    display: inline-block;
    padding-inline-end: 16px;
  }

  main {
    display: flex;
    flex-direction: column;
  }

  .content {
    flex: 1 1 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;

    border-top: 1px solid lightgray;
  }

  @media (min-width: 768px) {
    .content {
      flex-direction: row;
    }
  }
  .editor-container {
    flex: 1 1 0;
    min-height: 0;
    min-width: 0;
  }
  :global(.editor) {
    width: 100%;
    height: 100%;
  }
  .divider {
    flex: 0 0 1px;
    background-color: lightgray;
  }
</style>
