import {readFile} from 'node:fs/promises'
const cfg = JSON.parse(await readFile('C:/Users/Lenovo/.mlai-bridge/config.json', 'utf8'))
const BASE = 'http://127.0.0.1:' + (process.env.BRIDGE_PORT || '8790')
const KEY = process.env.BRIDGE_KEY || cfg.apiKey
const H = {'Content-Type': 'application/json', Authorization: 'Bearer ' + KEY}
const results = []
const check = (name, ok, detail) => results.push({name, ok: Boolean(ok), detail})
const json = async (p, opts) => { const r = await fetch(BASE + p, opts); const t = await r.text(); let d; try { d = JSON.parse(t) } catch { d = {raw: t.slice(0,200)} } return {status: r.status, d} }

const health = await json('/health')
check('health 200', health.status === 200 && health.d.ok === true, health.status)
check('health reports service', health.d.service === 'mlai-bridge' && health.d.ok === true, health.d.service)

const models = await json('/v1/models')
check('models returns aliases', models.status === 200 && models.d.data.some(m => m.id === 'glm'), models.d.data?.length)


const bad = await json('/v1/chat/completions', {method: 'POST', headers: {'Content-Type': 'application/json', Authorization: 'Bearer nope'}, body: JSON.stringify({model: 'glm', messages: [{role: 'user', content: 'hi'}]})})
check('wrong key 401', bad.status === 401 && bad.d.error?.code === 'invalid_api_key', bad.status)

const unknown = await json('/v1/chat/completions', {method: 'POST', headers: H, body: JSON.stringify({model: 'no-such', messages: [{role: 'user', content: 'hi'}]})})
check('unknown model 404', unknown.status === 404 && unknown.d.error?.code === 'model_not_found', unknown.status)

const ns = await json('/v1/chat/completions', {method: 'POST', headers: H, body: JSON.stringify({model: 'glm', messages: [{role: 'user', content: 'Reply with exactly: RUST-OK'}], max_tokens: 64, temperature: 0})})
const msg = ns.d.choices?.[0]?.message || {}
check('non-stream 200', ns.status === 200, ns.status)
check('non-stream alias model', ns.d.model === 'glm', ns.d.model)
check('non-stream content', typeof msg.content === 'string' && msg.content.length > 0, JSON.stringify(msg.content))
check('non-stream usage', Number(ns.d.usage?.total_tokens) > 0, ns.d.usage?.total_tokens)

const r = await fetch(BASE + '/v1/chat/completions', {method: 'POST', headers: {...H, Accept: 'text/event-stream'}, body: JSON.stringify({model: 'opus', stream: true, max_tokens: 260,
  tools: [{type: 'function', function: {name: 'ping', description: 'Ping', parameters: {type: 'object', properties: {host: {type: 'string'}}, required: ['host']}}}],
  messages: [{role: 'user', content: 'Call ping once with host example.com, then stop.'}]})})
const raw = await r.text()
const lines = raw.split('\n').filter(l => l.startsWith('data:'))
const frames = lines.map(l => l.slice(5).trim()).filter(t => t !== '[DONE]').map(t => { try { return JSON.parse(t) } catch { return null } }).filter(Boolean)
const toolChunks = frames.flatMap(f => (f.choices || []).flatMap(c => c.delta?.tool_calls || []))
const usageFrame = frames.find(f => f.usage && f.usage.total_tokens)
check('stream 200 SSE', r.status === 200 && String(r.headers.get('content-type')).includes('text/event-stream'), r.status)
check('stream ends [DONE]', lines.some(l => l.trim() === 'data: [DONE]'))
check('stream one id', new Set(frames.map(f => f.id)).size === 1, frames.length + ' frames')
check('stream alias model', frames.every(f => f.model === 'opus'))
check('stream chunk object', frames.every(f => f.object === 'chat.completion.chunk'))
check('stream tool index', toolChunks.length > 0 && toolChunks.every(t => Number.isInteger(t.index)), toolChunks.length)
check('stream tool name', toolChunks.map(t => t.function?.name).filter(Boolean).includes('ping'))
check('stream tool args', toolChunks.map(t => t.function?.arguments || '').join('').includes('example.com'))
check('stream usage chunk', Number(usageFrame?.usage?.total_tokens) > 0, usageFrame?.usage?.total_tokens)

const after = await json('/v1/models')
check('models carries context after mint', after.d.data.find(m => m.id === 'opus')?.context_length > 0, after.d.data.find(m => m.id === 'opus')?.context_length)
check('aliased model exposes 1M context', after.d.data.find(m => m.id === 'opus')?.context_length === 1048576, after.d.data.find(m => m.id === 'opus')?.context_length)
check('forge exposes 110k', after.d.data.find(m => m.id === 'forge')?.context_length === 110000, after.d.data.find(m => m.id === 'forge')?.context_length)

// usage accounting: two identical large prompts should register cache hits
const big = 'CACHE PROBE '.repeat(600)
for (let i = 0; i < 2; i++) {
  await fetch(BASE + '/v1/chat/completions', {method: 'POST', headers: H, body: JSON.stringify({model: 'opus', max_tokens: 8, temperature: 0, messages: [{role: 'user', content: big}]})})
}
const usage = await json('/usage')
const urows = usage.d.byModel || []
const opusRow = urows.find(r => r.model === 'opus')
check('usage endpoint returns totals', usage.status === 200 && usage.d.stats.requests > 0, usage.d.stats?.requests)
check('usage tracks input/output per model', !!opusRow && opusRow.promptTokens > 0 && opusRow.completionTokens > 0, opusRow ? opusRow.promptTokens + ' in / ' + opusRow.completionTokens + ' out' : 'missing opus row')
check('usage tracks cached input', !!opusRow && Number(opusRow.cachedTokens) > 0, opusRow?.cachedTokens)
check('usage tracks cache hits', !!opusRow && opusRow.cacheHits > 0, opusRow?.cacheHits)
check('usage reports credits', usage.d.tokensPerCredit === 1000000 && typeof usage.d.stats.credits === 'number', usage.d.stats?.credits)
check('credits match tokens', Math.abs((usage.d.stats.totalTokens / 1000000) - usage.d.stats.credits) < 0.001, usage.d.stats?.totalTokens)

const failed = results.filter(x => !x.ok)
for (const x of results) process.stdout.write((x.ok ? 'PASS ' : 'FAIL ') + x.name + (x.detail !== undefined && x.detail !== '' ? '  [' + String(x.detail).slice(0,50) + ']' : '') + '\n')
process.stdout.write('\n' + (results.length - failed.length) + '/' + results.length + ' checks passed\n')
process.exit(failed.length ? 1 : 0)
