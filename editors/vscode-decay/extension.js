const vscode = require('vscode');
const { LanguageClient } = require('vscode-languageclient/node');

let client;

function activate(context) {
  const command = vscode.workspace.getConfiguration('decay').get('server.path', 'decay-lsp');
  const serverOptions = {
    command,
    args: []
  };
  const clientOptions = {
    documentSelector: [{ scheme: 'file', language: 'decay' }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher('**/*.{decay,scene.json}')
    }
  };
  client = new LanguageClient('decay', 'Decay Language Server', serverOptions, clientOptions);
  context.subscriptions.push(client.start());
}

async function deactivate() {
  if (client) {
    await client.stop();
  }
}

module.exports = { activate, deactivate };
