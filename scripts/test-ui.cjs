const { chromium } = require('playwright');
const assert = require('node:assert/strict');
(async () => {
  const browser = await chromium.launch({ channel: 'msedge', headless: true });
  const page = await browser.newPage({ viewport: { width: 960, height: 640 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const items = Array.from({length: 40}, (_, i) => ({id: `test-${i}`, kind: 'text', content: `### Record ${i}\n\n` + ('Long paragraph 中文内容 https://example.com/verylonglink 文本换行测试。\n\n'.repeat(30)) + `END-${i}`, preview: `Record ${i} preview`, isStar: false, createdAt: 1780000000, updatedAt: 1780000000}));
  await page.route('**/src/tauriClient.ts', async route => {
    const response = await route.fetch();
    let body = await response.text();
    body = body.replace(/const demoItems = \[[\s\S]*?\n\];/, `const demoItems = ${JSON.stringify(items)};`);
    body = body.replace('export async function onNewItem(callback) {', 'export async function onNewItem(callback) { window.__newClipboardItem = callback;');
    body = body.replace('export async function call(command, args) {', `export async function call(command, args) {
      if (command === 'search_local_light') {
        await new Promise(resolve => setTimeout(resolve, args.keyword === 'slow-old' ? 600 : 30));
        return demoItems.filter(item => (item.content + item.preview).includes(args.keyword)).map(({content, ...item}) => item);
      }
    `);
    await route.fulfill({ response, body });
  });
  await page.goto(process.env.UI_TEST_URL || 'http://127.0.0.1:1420');
  await page.locator('.historyItem').first().waitFor();
  async function checkLayout(label) {
    await page.waitForTimeout(180);
    const rows = await page.locator('.historyRow').evaluateAll(nodes => nodes.map(node => ({index: Number(node.dataset.index), top: node.getBoundingClientRect().top, bottom: node.getBoundingClientRect().bottom, articleBottom: node.firstElementChild.getBoundingClientRect().bottom})).sort((a,b)=>a.index-b.index));
    for (let i = 1; i < rows.length; i++) {
      assert.ok(Math.abs(rows[i].top - rows[i-1].bottom) < 1, `${label}: row gap/overlap ${JSON.stringify(rows.slice(i-1,i+1))}`);
      assert.ok(rows[i].top - rows[i-1].articleBottom >= 9.5, `${label}: missing spacing`);
    }
    console.log('PASS', label, rows.length, 'mounted rows');
  }
  await checkLayout('initial');
  const first = page.locator('.historyItem').first();
  await first.getByRole('button', {name:'展开全部'}).click();
  await page.waitForFunction(() => document.querySelector('.recordContent.expanded')?.textContent.includes('END-0'));
  assert.ok(await page.locator('.recordContent.expanded').first().evaluate(el => el.clientHeight > 220));
  await checkLayout('expand fetched full text');
  await first.getByRole('button', {name:'收起'}).click();
  assert.ok(await page.locator('.recordContent').first().evaluate(el => el.clientHeight <= 220));
  await checkLayout('collapse long text');
  await first.getByRole('button', {name:'展开全部'}).focus();
  await page.keyboard.press('Enter');
  await checkLayout('keyboard expand');
  await first.getByRole('button', {name:'收起'}).click();
  await page.setViewportSize({width:760, height:520});
  await checkLayout('narrow window');
  await page.getByTitle('折叠侧边栏').click();
  await checkLayout('sidebar width change');
  await page.locator('.historyList').evaluate(el => el.scrollTop = 1800);
  await checkLayout('virtual scroll');
  await page.getByPlaceholder('搜索剪贴板', {exact:true}).fill('Record 2');
  await page.waitForTimeout(400);
  await checkLayout('filtered indices');
  await page.getByPlaceholder('搜索剪贴板', {exact:true}).fill('');
  await page.waitForTimeout(400);
  await checkLayout('restore history');
  await page.locator('.historyList').evaluate(el => el.scrollTop = 0);
  await page.evaluate(item => window.__newClipboardItem(item), {...items[0], id:'new-record'});
  await checkLayout('prepend full long record');
  assert.ok(await page.locator('.recordContent').first().evaluate(el => el.clientHeight <= 220));
  await page.locator('.historyItem').first().getByRole('button', {name:'展开全部'}).click();
  await checkLayout('expand new record');
  await page.locator('.historyItem').first().getByRole('button', {name:'收起'}).click();
  await page.evaluate(item => window.__newClipboardItem(item), items[20]);
  await checkLayout('existing record moved to front');
  const search = page.locator('.searchBox input');
  await search.fill('END-25');
  await page.waitForTimeout(400);
  assert.equal(await page.locator('.historyItem').count(), 1, 'full-content search must survive missing preview match');
  assert.ok((await page.locator('.historyItem').innerText()).includes('Record 25 preview'));
  await search.fill('slow-old');
  await page.waitForTimeout(300);
  await search.fill('END-26');
  await page.waitForTimeout(900);
  assert.ok((await page.locator('.historyItem').innerText()).includes('Record 26 preview'), 'stale response must not replace new results');
  await search.fill('slow-old');
  await page.waitForTimeout(300);
  await page.locator('.searchClearButton').click();
  await page.waitForTimeout(800);
  assert.ok(await page.locator('.historyItem').count() > 1, 'clear search must invalidate pending requests');
  console.log('PASS full-text matches, response race, clear race');
  await page.emulateMedia({reducedMotion:'reduce'});
  assert.equal(await page.locator('.historyItem').first().evaluate(el => getComputedStyle(el).transitionDuration), '0s');
  console.log('PASS reduced motion');
  assert.deepEqual(errors, []);
  await browser.close();
})().catch(error => { console.error(error); process.exit(1); });
