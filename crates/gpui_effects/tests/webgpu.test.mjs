import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import test from 'node:test';

async function glassShaders() {
    const read = path => readFile(new URL(path, import.meta.url), 'utf8');
    const template = await read('../../gpui/src/backdrop.wgsl');
    const shape = await read('../src/surface/shaders/liquid_glass_shape.wgsl');
    const shaders = [];
    for (const sampling of ['backdrop_sampling', 'backdrop_sampling_manual']) {
        const sampler = await read(`../../gpui/src/${sampling}.wgsl`);
        for (const name of ['frosted_glass', 'liquid_glass', 'fluted_glass']) {
            const effect = await read(`../src/surface/shaders/${name}.wgsl`);
            shaders.push({
                name: `${name}/${sampling}`,
                code: template.replace('// __GPUI_BACKDROP_SAMPLING__', sampler)
                    .replace('// __GPUI_BACKDROP_SOURCE__',
                        (name === 'liquid_glass' ? shape + '\n' : '') + effect),
            });
        }
    }
    return shaders;
}

test('glass shaders create WebGPU pipelines with hardware and manual sampling', { timeout: 30000 }, async () => {
    const shaders = await glassShaders();
    const profile = await mkdtemp(join(tmpdir(), 'gpui-glass-webgpu-'));
    let chrome;
    let socket;
    try {
        const page = join(profile, 'test.html');
        await writeFile(page, '<!doctype html><meta charset="utf-8"><title>WebGPU validation</title>');
        chrome = spawn(process.env.CHROME || 'google-chrome', [
            '--headless=new', '--no-sandbox', '--no-first-run', '--disable-background-networking',
            '--enable-unsafe-webgpu', '--use-angle=swiftshader', '--remote-debugging-port=0',
            '--user-data-dir=' + profile, pathToFileURL(page).href,
        ], { stdio: 'ignore' });
        await once(chrome, 'spawn');
        let port;
        for (let attempt = 0; attempt < 100; attempt++) {
            assert.equal(chrome.exitCode, null, 'browser exited before exposing its debugger');
            try {
                port = (await readFile(join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0];
                break;
            } catch { await delay(50); }
        }
        assert.ok(port, 'browser debugger did not start');
        const pages = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
        socket = new WebSocket(pages.find(page => page.type === 'page').webSocketDebuggerUrl);
        await once(socket, 'open');
        const response = new Promise((resolve, reject) => {
            const timeout = setTimeout(() => reject(Error('WebGPU validation timed out')), 15000);
            socket.addEventListener('message', event => {
                const reply = JSON.parse(event.data);
                if (reply.id === 1) {
                    clearTimeout(timeout);
                    resolve(reply);
                }
            });
        });
        socket.send(JSON.stringify({
            id: 1, method: 'Runtime.evaluate', params: {
                awaitPromise: true, returnByValue: true,
                expression: `(${async function validate(shaders) {
                    const adapter = await navigator.gpu.requestAdapter();
                    if (!adapter) throw Error('WebGPU adapter unavailable');
                    const device = await adapter.requestDevice();
                    try {
                        const results = [];
                        for (const { name, code } of shaders) {
                            const module = device.createShaderModule({ code, label: name });
                            const info = await module.getCompilationInfo();
                            const errors = Array.from(info.messages).filter(message => message.type === 'error');
                            if (errors.length) throw Error(name + ': ' + errors.map(error => error.message).join('\n'));
                            await device.createRenderPipelineAsync({
                                layout: 'auto',
                                vertex: { module, entryPoint: 'vs_backdrop' },
                                fragment: { module, entryPoint: 'fs_backdrop', targets: [{ format: 'rgba8unorm' }] },
                                primitive: { topology: 'triangle-strip' },
                            });
                            results.push(name);
                        }
                        return results;
                    } finally { device.destroy(); }
                }})(${JSON.stringify(shaders)})`,
            },
        }));
        const reply = await response;
        assert.ok(!reply.error && !reply.result?.exceptionDetails, JSON.stringify(reply));
        assert.deepEqual(reply.result.result.value, shaders.map(shader => shader.name));
    } finally {
        socket?.close();
        if (chrome && chrome.exitCode === null) {
            const exited = once(chrome, 'exit');
            chrome.kill();
            await exited;
        }
        await rm(profile, { recursive: true, force: true, maxRetries: 5 });
    }
});
