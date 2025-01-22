<script lang="ts">
  import CodeMirror from "svelte-codemirror-editor";
  import { javascript } from "@codemirror/lang-javascript";
  import { EditorView } from "codemirror";
  import { transpile } from "oxidase";

  const jsLang = javascript();
  const tsLang = javascript({ typescript: true });
  const theme = EditorView.theme({
    "&.cm-editor": { height: "100%" },
    ".cm-scroller": { overflow: "auto" },
  });

  let source = $state("");

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
  <h1>Oxidase Playground</h1>

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
  h1 {
    text-align: center;
  }
  main {
    display: flex;
    flex-direction: column;
  }

  .content {
    flex: 1 1 0;
    display: flex;
    flex-direction: column;
    border-top: 1px solid lightgray;
  }

  @media (min-width: 768px) {
    .content {
      flex-direction: row;
    }
  }
  .editor-container {
    flex: 1 1 0;
    overflow: scroll;
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
