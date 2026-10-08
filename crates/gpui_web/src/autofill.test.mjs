import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import test from 'node:test';

const page = `<!doctype html><meta charset="utf-8"><body><script type="module">
import { WebAutofill } from '/autofill.js';
try {
    const check = (condition, message) => { if (!condition) throw Error(message); };
    const canvas = document.createElement('canvas');
    canvas.style.cssText = 'position:absolute;left:20px;top:30px;width:600px;height:400px';
    const ime = document.createElement('input');
    document.body.append(canvas, ime);
    ime.focus();
    const fills = [];
    const focuses = [];
    const host = new WebAutofill(canvas, ime, (id, value) => fills.push([id, value]), id => focuses.push(id));
    let inUpdate = false;
    let reentered = false;
    for (const event of ['focus', 'blur']) host.listen(event, () => { reentered ||= inUpdate; });
    const fields = [
        {id:'9007199254740993',name:'login-username',hint:'username',value:'',focused:true,x:10,y:20,width:200,height:30},
        {id:'9007199254740994',name:'login-password',hint:'current-password',value:'',focused:false,x:10,y:70,width:200,height:30},
    ];
    const update = () => { inUpdate = true; host.update(JSON.stringify(fields), 600, 400); inUpdate = false; };
    update(); await Promise.resolve();
    const username = host.form.elements.namedItem('login-username');
    const password = host.form.elements.namedItem('login-password');
    check(username.autocomplete === 'username' && password.autocomplete === 'current-password' && password.type === 'password', 'missing browser semantics');
    check(document.activeElement === username, 'focused field did not become the input target');
    const rect = username.getBoundingClientRect();
    check(rect.left === canvas.getBoundingClientRect().left + 10 && rect.width === 200, 'field geometry');
    check(document.elementFromPoint(rect.left + 5, rect.top + 5) === username, 'canvas intercepts password-manager clicks');
    canvas.addEventListener('pointerdown', () => host.focus());
    host.form.dispatchEvent(new PointerEvent('pointerdown', {bubbles:true, cancelable:true}));
    update(); await Promise.resolve();
    check(document.activeElement === ime, 'outside click did not dismiss native field focus across redraw');
    username.focus(); update(); await Promise.resolve();
    check(document.activeElement === username, 'field could not regain native focus after dismissal');
    username.value = 'demo@example.test';
    password.value = 'test-only-password';
    username.dispatchEvent(new InputEvent('input', {bubbles:true, inputType:'insertReplacementText'}));
    password.dispatchEvent(new Event('change', {bubbles:true}));
    update(); // A stale draw must not erase a fill whose callback is still queued.
    await Promise.resolve();
    check(fills.length === 2 && fills[0][0] === fields[0].id && fills[1][0] === fields[1].id, 'lost or repeated multi-field fill');
    check(password.value === 'test-only-password', 'pending password was erased');
    fields[0].value = fills[0][1]; fields[1].value = fills[1][1];
    update(); update(); await Promise.resolve();
    check(fills.length === 2 && host.form.elements.namedItem('login-username') === username, 'redraw replayed fill or replaced node');
    host.selection(1, 3);
    username.dispatchEvent(new CompositionEvent('compositionstart', {bubbles:true}));
    username.value = '预编辑';
    username.dispatchEvent(new InputEvent('input', {bubbles:true,isComposing:true,inputType:'insertCompositionText'}));
    update();
    check(fills.length === 2 && username.value === '预编辑', 'composition became an autofill commit');
    username.value = fields[0].value;
    username.dispatchEvent(new CompositionEvent('compositionend', {bubbles:true,data:''}));
    await Promise.resolve();
    password.focus();
    check(focuses.length === 0, 'DOM focus callback reentered synchronously');
    await Promise.resolve();
    check(focuses.length === 1 && focuses[0] === fields[1].id, 'native field focus was not reported');
    fields[0].focused = false; fields[1].focused = true; update(); await Promise.resolve();
    check(document.activeElement === password && username.value === fields[0].value, 'focus switch lost a field');
    fields.pop(); update(); await Promise.resolve();
    check(!password.isConnected && password.value === '', 'removed field retained a credential');
    check(document.activeElement === ime, 'removing the focused field stranded keyboard focus');
    check(!reentered, 'DOM focus reentered GPUI during a draw');
    password.value = 'stale'; password.dispatchEvent(new Event('change', {bubbles:true}));
    await Promise.resolve(); check(fills.length === 2, 'removed field accepted a fill');
    host.dispose();
    check(!host.form.isConnected && username.value === '', 'dispose retained credentials');
    document.body.dataset.result = 'passed';
} catch (error) { document.body.dataset.result = String(error); }
</script>`;

test('autofill preserves identities, multi-field values and IME boundaries', async () => {
    const sources = new Map(await Promise.all(['autofill.js', 'ime.js'].map(async name =>
        ['/' + name, await readFile(new URL('./' + name, import.meta.url))])));
    const server = createServer((req, res) => {
        res.setHeader('Content-Type', sources.has(req.url) ? 'text/javascript' : 'text/html');
        res.end(sources.get(req.url) || page);
    });
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    const profile = await mkdtemp(join(tmpdir(), 'gpui-autofill-'));
    try {
        const { stdout } = await promisify(execFile)(process.env.CHROME || 'google-chrome', [
            '--headless=new', '--no-sandbox', '--disable-gpu', '--no-first-run',
            '--disable-background-networking', '--password-store=basic',
            '--user-data-dir=' + profile, '--virtual-time-budget=1000', '--dump-dom',
            'http://127.0.0.1:' + server.address().port,
        ], { timeout:20000, maxBuffer:1024*1024 });
        assert.equal(stdout.match(/data-result="([^"]*)"/)?.[1], 'passed');
    } finally {
        server.closeAllConnections();
        await new Promise(resolve => server.close(resolve));
        await rm(profile, {recursive:true, force:true, maxRetries:5});
    }
});
