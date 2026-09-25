// Explicit opt-in: runs against a temporary, isolated native test instance.
// Launch it with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223.
// Does not print database records, API keys, or settings. Tests synthetic records only.
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const path = require('node:path');
const http = require('node:http');

(async () => {
  if (process.env.CLIPBOARD_NATIVE_TEST !== '1') throw new Error('Set CLIPBOARD_NATIVE_TEST=1 for an isolated test instance');
  const browser = await chromium.connectOverCDP('http://127.0.0.1:9223');
  let page;
  for(let i=0;i<30 && !page;i++) {
    page = browser.contexts()[0].pages().find(p => p.url().includes('tauri.localhost'));
    if(!page) await new Promise(resolve=>setTimeout(resolve,100));
  }
  assert.ok(page, 'Native WebView not found');
  await page.waitForFunction(() => window.__TAURI_INTERNALS__);
  const invoke = (command, args) => page.evaluate(([c,a]) => window.__TAURI_INTERNALS__.invoke(c,a), [command,args]);
  const original = await invoke('get_app_settings');
  assert.equal((await invoke('get_history_light', {limit:0,offset:0})).length, 0, 'Native tests require an EMPTY temporary database');
  const ids = new Set();
  let testFolder;
  let reply = 'ok';
  let requests = 0;
  const server = http.createServer((req,res) => {
    requests++;
    req.resume();
    req.on('end', () => {
      res.writeHead(200, {'Content-Type':'application/json'});
      res.end(JSON.stringify(req.url.endsWith('/models') ? {data:[{id:'test-model'}]} : req.url.endsWith('/messages')
        ? {content:[{type:'text',text:reply}]} : {choices:[{message:{content:reply}}]}));
    });
  });
  await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  const helper = path.resolve('scripts/native-clipboard-test.py');
  const put = (...args) => execFileSync('python',[helper,...args],{windowsHide:true});
  async function waitForHistory(predicate) {
    for(let i=0;i<40;i++) {
      const rows=await invoke('get_history_light',{limit:0,offset:0});
      if(predicate(rows)) return rows;
      await new Promise(r=>setTimeout(r,100));
    }
    throw new Error('Clipboard capture timed out');
  }
  try {
    await invoke('save_app_settings',{settings:{...original,captureEnabled:true,runAtStartup:false,apiKey:''}});
    const marker='SC-AUTOTEST-'+Date.now();
    put('text',marker+' '+('Long paragraph test. '.repeat(150))+' END-MARKER');
    const text=(await waitForHistory(rows=>rows.some(r=>r.kind==='text')))[0]; ids.add(text.id);
    assert.equal(text.content,null);
    assert.ok((await invoke('get_item',{id:text.id})).content.endsWith('END-MARKER'));
    assert.equal((await invoke('search_local_light',{keyword:'END-MARKER'}))[0].id,text.id);
    await page.locator('.historyItem').first().getByRole('button',{name:'展开全部'}).click();
    assert.ok((await page.locator('.recordContent.expanded').innerText()).includes('END-MARKER'));
    await page.locator('.historyItem').first().getByRole('button',{name:'收起'}).click();
    assert.ok(await page.locator('.recordContent').first().evaluate(el=>el.clientHeight<=220));
    await invoke('update_item_text',{id:text.id,text:marker+' edited'});
    assert.equal((await invoke('get_item',{id:text.id})).content,marker+' edited');
    assert.equal((await invoke('toggle_star',{id:text.id,isStar:true})).isStar,true);
    testFolder=await invoke('create_folder',{name:marker});
    assert.equal((await invoke('move_to_folder',{itemId:text.id,folderId:testFolder.id})).folderId,testFolder.id);
    await invoke('delete_folder',{id:testFolder.id});testFolder=null;
    assert.equal((await invoke('get_item',{id:text.id})).folderId,null);
    console.log('PASS native capture, full text, search, expand/collapse, edit, star, folders');

    put('image',path.resolve('src-tauri/tests/fixtures/ocr-smoke.png'));
    const image=(await waitForHistory(rows=>rows.some(r=>r.kind==='image'))).find(r=>r.kind==='image');ids.add(image.id);
    await page.locator('.imageThumbButton img').first().waitFor();
    assert.ok(await page.locator('.imageThumbButton img').first().evaluate(async img=>{await img.decode();return img.naturalWidth>0 && img.src.startsWith('http://asset.localhost/')}));
    const requestCount=requests;
    const started=performance.now();
    const recognized=await invoke('trigger_ocr',{imageId:image.id});
    const elapsed=Math.round(performance.now()-started);
    assert.ok(recognized.ocrText.replace(/\s/g,'').includes('剪贴板OCR测试123ABC'));
    assert.ok(recognized.ocrText.includes('\n'));
    assert.equal(requests,requestCount,'OCR must not call AI');
    assert.ok((await invoke('search_local_light',{keyword:'second line'})).some(r=>r.id===image.id));
    assert.equal((await invoke('get_item',{id:image.id})).ocrText,recognized.ocrText);
    console.log(`PASS native local image protocol, OCR without key, multiline, persistence/search (${elapsed} ms)`);
    await invoke('execute_paste',{itemId:text.id});
    put('assert-text',marker+' edited');
    await invoke('execute_paste',{itemId:'',overrideText:'OCR target pasted'});
    put('assert-text','OCR target pasted');
    await invoke('execute_paste',{itemId:image.id});
    put('assert-image');
    console.log('PASS native paste command sets history text, OCR text, and image clipboard formats');

    for(const aiProtocol of ['openai','anthropic']) {
      await invoke('save_app_settings',{settings:{...original,runAtStartup:false,aiProtocol,apiKey:'synthetic-test-key',openaiBaseUrl:base+'/v1',anthropicBaseUrl:base,searchModel:'test-model'}});
      reply='ok'; assert.ok((await invoke('test_ai_connection')).includes('ok'));
      assert.ok((await invoke('list_ai_models')).includes('test-model'));
      reply=JSON.stringify({ids:[text.id]});
      assert.deepEqual(await invoke('search_ai_semantic',{query:marker}),[text.id]);
      reply=JSON.stringify({assignments:[{id:text.id,folder:marker}]});
      assert.ok((await invoke('trigger_ai_categorize')).some(r=>r.id===text.id && r.folderId));
      testFolder=(await invoke('get_folders')).find(f=>f.name===marker);
      await invoke('delete_folder',{id:testFolder.id});testFolder=null;
    }
    console.log('PASS native OpenAI/Anthropic connection, models, semantic search and archive against loopback fixture');
    await invoke('save_app_settings',{settings:{...original,captureEnabled:false,runAtStartup:false}});
    const count=(await invoke('get_history_light',{limit:0,offset:0})).length;
    put('text',marker+' capture-disabled');await new Promise(r=>setTimeout(r,300));
    assert.equal((await invoke('get_history_light',{limit:0,offset:0})).length,count);
    console.log('PASS capture disabled');
  } finally {
    if(testFolder) await invoke('delete_folder',{id:testFolder.id}).catch(()=>{});
    for(const id of ids) await invoke('delete_item',{id}).catch(()=>{});
    await invoke('save_app_settings',{settings:original});
    await invoke('hide_window');
    await new Promise(resolve=>server.close(resolve));
    await browser.close();
  }
})().catch(e=>{console.error(e);process.exit(1)});
