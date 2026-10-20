(() => {
  'use strict';

  const DATA = JSON.parse(document.getElementById('ni-data').textContent);

  const VIEWS = ['board', 'arch', 'docs', 'timeline'];
  const STATUS_COLUMNS = ['backlog', 'ready', 'doing', 'review', 'done'];
  const STATUS_LABEL = {
    backlog: 'Backlog',
    ready: 'Ready',
    doing: 'Doing',
    review: 'Review',
    done: 'Done',
    blocked: 'Blocked',
    planned: 'Planned',
    active: 'Active',
  };
  const STATUS_COLORS = {
    backlog: '#8c8478',
    ready: '#3f6f9f',
    doing: '#b7791f',
    review: '#6f5aa8',
    done: '#3f7d4f',
    blocked: '#b42318',
  };
  const SIZE_POINTS = { S: 1, M: 2, L: 3 };
  const LAYER_COLORS = {
    interface: '#4c7bd9',
    engine: '#b57436',
    adapters: '#8768bf',
    state: '#5c8f4a',
    engineering: '#8a8175',
  };
  const KIND_LABEL = { adr: 'ADR', doc: 'Doc', guide: 'Guide' };

  const canvas = document.getElementById('canvas');
  const details = document.getElementById('details');
  const search = document.getElementById('search');
  const themeButton = document.getElementById('theme-toggle');

  const cardById = new Map(DATA.cards.map((card) => [card.id, card]));
  const moduleById = new Map(DATA.modules.map((module) => [module.id, module]));
  const docById = new Map(DATA.docs.map((doc) => [doc.id, doc]));
  const milestoneById = new Map(DATA.milestones.map((milestone) => [milestone.id, milestone]));
  const layerById = new Map(DATA.layers.map((layer) => [layer.id, layer]));

  const state = {
    view: 'board',
    milestone: currentMilestoneId(),
    selection: null,
    query: '',
    sourceIndex: 0,
  };

  let edgeFrame = 0;

  function currentMilestoneId() {
    const active = DATA.milestones.find((milestone) => milestone.status === 'active');
    return active ? active.id : 'all';
  }

  function esc(value) {
    return String(value ?? '').replace(/[&<>"']/g, (character) => ({
      '&': '&amp;',
      '<': '&lt;',
      '>': '&gt;',
      '"': '&quot;',
      "'": '&#39;',
    }[character]));
  }

  function fmtDate(value) {
    return value ? value.slice(5) : '—';
  }

  function docKind(kind) {
    return KIND_LABEL[kind] || String(kind || 'doc').toUpperCase();
  }

  function layerColor(id) {
    return LAYER_COLORS[id] || '#8a8175';
  }

  function layerName(id) {
    const layer = layerById.get(id);
    return layer ? layer.name : id;
  }

  function moduleTitle(id) {
    const module = moduleById.get(id);
    return module ? module.title : id;
  }

  function statusBadge(status) {
    const label = STATUS_LABEL[status] || status;
    return `<span class="badge badge-${esc(status)}"><span class="dot"></span>${esc(label)}</span>`;
  }

  function milestoneStats(id) {
    const cards = DATA.cards.filter((card) => id === 'all' || card.milestone === id);
    const done = cards.filter((card) => card.status === 'done');
    const points = cards.reduce((sum, card) => sum + (SIZE_POINTS[card.size] || 1), 0);
    const donePoints = done.reduce((sum, card) => sum + (SIZE_POINTS[card.size] || 1), 0);
    return {
      total: cards.length,
      done: done.length,
      points,
      donePoints,
      pct: points ? Math.round((donePoints / points) * 100) : 0,
    };
  }

  function isCardSelected(id) {
    return state.selection?.type === 'card' && state.selection.id === id;
  }

  function isModuleSelected(id) {
    return state.selection?.type === 'module' && state.selection.id === id;
  }

  function isDocSelected(id) {
    return state.selection?.type === 'doc' && state.selection.id === id;
  }

  function cardMatches(card, query) {
    if (!query) return true;
    return [card.id, card.title, card.module, card.status, card.priority]
      .filter(Boolean)
      .join(' ')
      .toLowerCase()
      .includes(query);
  }

  function moduleMatches(module, query) {
    if (!query) return true;
    return [module.id, module.title, module.summary, module.layer, module.status]
      .filter(Boolean)
      .join(' ')
      .toLowerCase()
      .includes(query);
  }

  function docMatches(doc, query) {
    if (!query) return true;
    return [doc.id, doc.title, doc.kind, doc.status, doc.summary, doc.excerpt, doc.path]
      .filter(Boolean)
      .join(' ')
      .toLowerCase()
      .includes(query);
  }

  function eventMatches(event, query) {
    if (!query) return true;
    return [event.date, event.kind, event.title, event.detail, ...(event.refs || [])]
      .filter(Boolean)
      .join(' ')
      .toLowerCase()
      .includes(query);
  }

  function viewHead(kicker, title, subtitle, stats) {
    return `<div class="view-head">
      <div>
        <div class="kicker">${esc(kicker)}</div>
        <h1 class="view-title">${esc(title)}</h1>
        ${subtitle ? `<p class="view-sub">${esc(subtitle)}</p>` : ''}
      </div>
      ${stats ? `<div class="stats">${stats}</div>` : ''}
    </div>`;
  }

  function compareCards(a, b) {
    const priority = { P0: 0, P1: 1, P2: 2 };
    const pa = priority[a.priority] ?? 9;
    const pb = priority[b.priority] ?? 9;
    return pa - pb || a.id.localeCompare(b.id);
  }

  function cardHtml(card) {
    const module = card.module ? moduleById.get(card.module) : null;
    const selected = isCardSelected(card.id);
    return `<button class="card${card.status === 'done' ? ' done' : ''}${card.status === 'blocked' ? ' blocked' : ''}${selected ? ' selected' : ''}" type="button" data-card="${esc(card.id)}" aria-pressed="${selected}">
      <span class="card-top"><span>${esc(card.id)}</span><span class="size">${esc(card.size)}</span></span>
      <span class="card-title">${esc(card.title)}</span>
      <span class="card-foot"><span>${module ? esc(module.title) : '—'}</span><span>${esc(fmtDate(card.updated))}</span></span>
    </button>`;
  }

  function renderMilestones() {
    const allStats = milestoneStats('all');
    let html = `<button class="milestone${state.milestone === 'all' ? ' active' : ''}" type="button" data-milestone="all" aria-pressed="${state.milestone === 'all'}">
      <span class="milestone-top"><span class="ms-dot"></span><span class="ms-id">ALL</span><span class="ms-title">Every milestone</span></span>
      <span class="ms-meta"><span>${allStats.done}/${allStats.total}</span><span>${allStats.pct}%</span></span>
      <span class="progress"><i style="width:${allStats.pct}%"></i></span>
    </button>`;

    html += DATA.milestones.map((milestone) => {
      const stats = milestoneStats(milestone.id);
      return `<button class="milestone${state.milestone === milestone.id ? ' active' : ''}" type="button" data-milestone="${esc(milestone.id)}" aria-pressed="${state.milestone === milestone.id}">
        <span class="milestone-top"><span class="ms-dot ${esc(milestone.status)}"></span><span class="ms-id">${esc(milestone.id)}</span><span class="ms-title">${esc(milestone.title)}</span></span>
        <span class="ms-meta"><span>${stats.done}/${stats.total}</span><span>${stats.pct}%</span></span>
        <span class="progress"><i style="width:${stats.pct}%"></i></span>
      </button>`;
    }).join('');

    return html;
  }

  function renderLayers() {
    return DATA.layers.map((layer) => {
      const count = DATA.modules.filter((module) => module.layer === layer.id).length;
      return `<div class="layer-row"><span class="layer-dot" style="--c:${layerColor(layer.id)}"></span><span class="layer-name">${esc(layer.name)}</span><span class="layer-count">${count}</span></div>`;
    }).join('');
  }

  function renderRailDocs() {
    return DATA.docs.map((doc) => `<button class="rail-doc${isDocSelected(doc.id) ? ' active' : ''}" type="button" data-doc="${esc(doc.id)}" title="${esc(doc.title)}">
      <span class="rd-kind">${esc(docKind(doc.kind))}</span><span class="rd-title">${esc(doc.title)}</span>
    </button>`).join('');
  }

  function renderChrome() {
    document.getElementById('project-status').textContent = DATA.project.status;
    document.getElementById('milestone-count').textContent = DATA.milestones.length;
    document.getElementById('layer-count').textContent = DATA.layers.length;
    document.getElementById('doc-count').textContent = DATA.docs.length;

    const done = DATA.cards.filter((card) => card.status === 'done').length;
    document.getElementById('rail-foot').innerHTML = `<span>${done}/${DATA.cards.length} cards done</span><span>${DATA.modules.length} modules · ${DATA.docs.length} docs</span><span>offline · schema v${DATA.meta.schema_version}</span>`;

    const placeholders = {
      board: 'Filter cards',
      arch: 'Filter modules',
      docs: 'Filter documents',
      timeline: 'Filter events',
    };
    search.placeholder = placeholders[state.view] || 'Filter';
  }

  function renderTabs() {
    document.querySelectorAll('.tab').forEach((tab) => {
      const active = tab.dataset.view === state.view;
      tab.classList.toggle('active', active);
      if (active) tab.setAttribute('aria-current', 'page');
      else tab.removeAttribute('aria-current');
    });
  }

  function renderRail() {
    document.getElementById('milestone-list').innerHTML = renderMilestones();
    document.getElementById('layer-list').innerHTML = renderLayers();
    document.getElementById('rail-docs').innerHTML = renderRailDocs();
  }

  function renderCanvas() {
    const previousView = canvas.dataset.view || '';
    const previousScroll = canvas.scrollTop;
    let html = '';

    if (state.view === 'board') html = boardView();
    else if (state.view === 'arch') html = archView();
    else if (state.view === 'docs') html = docsView();
    else html = timelineView();

    canvas.innerHTML = html;
    canvas.dataset.view = state.view;
    canvas.scrollTop = previousView === state.view ? previousScroll : 0;

    if (state.view === 'arch') scheduleEdges();
  }

  function boardView() {
    const milestone = state.milestone === 'all' ? null : milestoneById.get(state.milestone);
    const stats = milestoneStats(state.milestone);
    const query = state.query.toLowerCase();
    const cards = DATA.cards.filter((card) => {
      const inMilestone = state.milestone === 'all' || card.milestone === state.milestone;
      return inMilestone && cardMatches(card, query);
    });
    const blocked = cards.filter((card) => card.status === 'blocked');

    const statsHtml = `<span><strong>${stats.done}/${stats.total}</strong> done</span><span class="sep">·</span><span>${stats.points} pts</span>`;
    const title = milestone ? `${milestone.id} · ${milestone.title}` : 'All milestones';
    const subtitle = milestone ? milestone.goal : 'Every milestone on one board.';

    let html = viewHead('Iteration board', title, subtitle, statsHtml);
    html += `<div class="progress wide" style="margin:-4px 0 15px"><i style="width:${stats.pct}%"></i></div>`;

    if (blocked.length) {
      html += `<div class="blocked-strip"><div class="blocked-label">Blocked · ${blocked.length}</div><div class="blocked-cards">${blocked.map(cardHtml).join('')}</div></div>`;
    }

    html += `<div class="board">${STATUS_COLUMNS.map((status) => boardColumn(status, cards)).join('')}</div>`;

    if (!cards.length) {
      html += `<div class="empty" style="margin-top:12px">No cards match this filter.</div>`;
    }

    return html;
  }

  function boardColumn(status, cards) {
    const items = cards.filter((card) => card.status === status).sort(compareCards);
    const warning = status === 'doing' && items.length > 3;
    return `<section class="column${items.length ? '' : ' empty'}" data-status="${status}">
      <div class="column-head"><span class="col-dot" style="--c:${STATUS_COLORS[status]}"></span>${esc(STATUS_LABEL[status])}<span class="col-count${warning ? ' warn' : ''}">${items.length}</span></div>
      <div class="cards">${items.map(cardHtml).join('')}</div>
    </section>`;
  }

  function archView() {
    const query = state.query.toLowerCase();
    const active = DATA.modules.filter((module) => module.status === 'active').length;
    const stats = `<span><strong>${DATA.modules.length}</strong> modules</span><span class="sep">·</span><span>${active} active</span>`;

    let html = viewHead(
      'Target architecture',
      'How ni is shaped',
      '语义分层，不是目录树。实线标出调用与数据方向；虚线卡片表示尚未落地。',
      stats,
    );
    html += `<div class="arch-canvas" id="arch-canvas"><svg class="edges" id="arch-edges" aria-hidden="true"></svg>`;

    DATA.layers.forEach((layer, index) => {
      const modules = DATA.modules.filter((module) => module.layer === layer.id);
      html += `<section class="arch-layer">
        <div class="layer-label"><div class="layer-index">${String(index + 1).padStart(2, '0')}</div><h2>${esc(layer.name)}</h2><p>${esc(layer.note)}</p></div>
        <div class="layer-cards">${modules.map((module) => moduleCard(module, query)).join('') || '<div class="empty">No modules yet.</div>'}</div>
      </section>`;
    });

    html += `</div>`;
    return html;
  }

  function moduleCard(module, query) {
    const selected = isModuleSelected(module.id);
    const dim = query && !moduleMatches(module, query);
    return `<button class="module-card${module.status === 'planned' ? ' planned' : ''}${selected ? ' selected' : ''}${dim ? ' dim' : ''}" type="button" data-module="${esc(module.id)}" style="--module-color:${layerColor(module.layer)}" aria-pressed="${selected}">
      <span class="module-head"><span>${esc(module.id)}</span><span class="module-status">${esc(STATUS_LABEL[module.status] || module.status)}</span></span>
      <span class="module-title">${esc(module.title)}</span>
      <span class="module-summary">${esc(module.summary)}</span>
      <span class="module-path">${esc(module.paths?.[0] || '—')}</span>
    </button>`;
  }

  function docsView() {
    const query = state.query.toLowerCase();
    const docs = DATA.docs.filter((doc) => docMatches(doc, query));
    const stats = `<span><strong>${DATA.docs.length}</strong> docs</span><span class="sep">·</span><span>${DATA.docs.filter((doc) => doc.kind === 'adr').length} ADRs</span>`;

    return viewHead(
      'Documents',
      'Decisions, architecture, rituals',
      'ADR 与指南和代码放在一起。点开在右栏阅读，深链可以直接分享。',
      stats,
    ) + `<div class="doc-grid">${docs.map(docCard).join('')}${docs.length ? '' : '<div class="empty">No documents match this filter.</div>'}</div>`;
  }

  function docCard(doc) {
    const selected = isDocSelected(doc.id);
    return `<button class="doc-card${selected ? ' selected' : ''}" type="button" data-doc="${esc(doc.id)}" aria-pressed="${selected}">
      <span class="doc-top"><span class="doc-kind">${esc(docKind(doc.kind))}</span>${doc.status ? `<span class="doc-status">${esc(doc.status)}</span>` : ''}</span>
      <span class="doc-title">${esc(doc.title)}</span>
      <span class="doc-excerpt">${esc(doc.excerpt)}</span>
      <span class="doc-path">${esc(doc.path)}</span>
    </button>`;
  }

  function timelineView() {
    const query = state.query.toLowerCase();
    const events = DATA.events
      .filter((event) => eventMatches(event, query))
      .slice()
      .sort((a, b) => b.date.localeCompare(a.date));
    const stats = `<span><strong>${DATA.events.length}</strong> events</span>`;

    return viewHead(
      'Timeline',
      'How the project got here',
      '决策、里程碑和阶段变化。手工事件为主，后续可以从 git log 自动补充。',
      stats,
    ) + `<div class="timeline">${events.map(timelineItem).join('')}${events.length ? '' : '<div class="empty">No events match this filter.</div>'}</div>`;
  }

  function timelineItem(event) {
    const refs = (event.refs || []).map((ref) => {
      if (cardById.has(ref)) return `<button class="ref-chip" type="button" data-card="${esc(ref)}">${esc(ref)}</button>`;
      if (docById.has(ref)) return `<button class="ref-chip" type="button" data-doc="${esc(ref)}">${esc(ref)}</button>`;
      return `<span class="ref-chip">${esc(ref)}</span>`;
    }).join('');

    return `<article class="tl-item">
      <div><span class="tl-date">${esc(event.date)}</span><span class="tl-kind">${esc(event.kind)}</span></div>
      <h3 class="tl-title">${esc(event.title)}</h3>
      ${event.detail ? `<p class="tl-detail">${esc(event.detail)}</p>` : ''}
      ${refs ? `<div class="tl-refs">${refs}</div>` : ''}
    </article>`;
  }

  function detailsHead(kicker, closable = true) {
    return `<div class="details-head"><span class="kicker">${esc(kicker)}</span>${closable ? '<button class="close" type="button" data-close aria-label="Close details">×</button>' : ''}</div>`;
  }

  function metaItem(key, value) {
    return `<div class="meta-item"><div class="meta-k">${esc(key)}</div><div class="meta-v">${value}</div></div>`;
  }

  function renderDetails() {
    const key = state.selection ? `${state.selection.type}:${state.selection.id}` : 'overview';
    const changed = details.dataset.key !== key;
    let html = '';

    if (!state.selection) {
      html = overviewDetails();
    } else if (state.selection.type === 'card') {
      const card = cardById.get(state.selection.id);
      html = card ? cardDetails(card) : overviewDetails();
    } else if (state.selection.type === 'module') {
      const module = moduleById.get(state.selection.id);
      html = module ? moduleDetails(module) : overviewDetails();
    } else {
      const doc = docById.get(state.selection.id);
      html = doc ? docDetails(doc) : overviewDetails();
    }

    details.innerHTML = html;
    details.dataset.key = key;
    if (changed) details.scrollTop = 0;

    details.querySelectorAll('a[href^="http"]').forEach((link) => {
      link.target = '_blank';
      link.rel = 'noopener noreferrer';
    });
  }

  function overviewDetails() {
    const active = DATA.milestones.find((milestone) => milestone.status === 'active') || DATA.milestones[0];
    const stats = milestoneStats(active ? active.id : 'all');

    let html = detailsHead(`Project · ${DATA.project.name}`, false);
    html += `<h1 class="details-title">${esc(DATA.project.title)}</h1>`;
    html += `<p class="view-sub" style="font-size:12px">${esc(DATA.project.summary)}</p>`;
    html += `<div class="overview-stats">
      <div class="overview-stat"><strong>${DATA.cards.filter((card) => card.status === 'done').length}</strong><span>done</span></div>
      <div class="overview-stat"><strong>${DATA.modules.length}</strong><span>modules</span></div>
      <div class="overview-stat"><strong>${DATA.docs.length}</strong><span>docs</span></div>
    </div>`;

    if (active) {
      html += `<div class="overview-block"><h3>${esc(active.id)} · ${esc(active.title)}</h3><p>${esc(active.goal)}</p><div class="progress wide" style="margin:8px 0 6px"><i style="width:${stats.pct}%"></i></div><div class="hint">${stats.done}/${stats.total} cards · ${stats.pct}%</div></div>`;
      if (active.exit?.length) {
        html += `<div class="detail-section"><h2>Exit criteria</h2><ul class="notes">${active.exit.map((item) => `<li>${esc(item)}</li>`).join('')}</ul></div>`;
      }
    }

    html += `<div class="detail-section"><h2>Read next</h2><div class="path-list">
      <button class="path-chip" type="button" data-view="arch">Architecture map</button>
      <button class="path-chip" type="button" data-doc="architecture">Architecture doc</button>
      <button class="path-chip" type="button" data-doc="readme">Dashboard guide</button>
    </div></div>`;

    return html;
  }

  function cardDetails(card) {
    const module = card.module ? moduleById.get(card.module) : null;
    const milestone = milestoneById.get(card.milestone);
    const docs = (card.docs || []).map((id) => docById.get(id)).filter(Boolean);

    let html = detailsHead(`Task · ${card.id}`);
    html += `<h1 class="details-title">${esc(card.title)}</h1>`;
    html += `<div class="badges">${statusBadge(card.status)}${card.priority ? `<span class="badge">${esc(card.priority)}</span>` : ''}<span class="badge">${esc(card.size)}</span></div>`;

    if (card.status === 'blocked' && card.blocked_reason) {
      html += `<div class="callout">Blocked · ${esc(card.blocked_reason)}</div>`;
    }

    html += `<div class="meta-grid">
      ${metaItem('Milestone', milestone ? `${esc(milestone.id)} · ${esc(milestone.title)}` : esc(card.milestone))}
      ${metaItem('Module', module ? `<button class="inline-link" type="button" data-module="${esc(module.id)}">${esc(module.title)}</button>` : '—')}
      ${metaItem('Updated', `<span class="mono">${esc(card.updated)}</span>`)}
      ${metaItem('Completed', card.completed ? `<span class="mono">${esc(card.completed)}</span>` : '—')}
    </div>`;

    if (card.depends_on?.length) {
      html += `<div class="detail-section"><h2>Depends on</h2><div class="path-list">${card.depends_on.map((id) => `<button class="path-chip" type="button" data-card="${esc(id)}">${esc(id)} · ${esc(cardById.get(id)?.title || '')}</button>`).join('')}</div></div>`;
    }

    html += `<div class="md">${card.body_html}</div>`;

    if (docs.length) {
      html += `<div class="detail-section"><h2>Documents</h2><div class="path-list">${docs.map((doc) => `<button class="path-chip" type="button" data-doc="${esc(doc.id)}">${esc(doc.title)}</button>`).join('')}</div></div>`;
    }

    html += `<p class="hint">${esc(card.path)}</p>`;
    return html;
  }

  function moduleDetails(module) {
    let html = detailsHead(`Module · ${module.id}`);
    html += `<h1 class="details-title">${esc(module.title)}</h1>`;
    html += `<div class="badges">${statusBadge(module.status)}<span class="badge">${esc(layerName(module.layer))}</span></div>`;
    html += `<p class="view-sub" style="font-size:12px">${esc(module.summary)}</p>`;

    if (module.contract) {
      html += `<div class="detail-section"><h2>Contract</h2><p>${esc(module.contract)}</p></div>`;
    }

    if (module.notes?.length) {
      html += `<div class="detail-section"><h2>Notes</h2><ul class="notes">${module.notes.map((note) => `<li>${esc(note)}</li>`).join('')}</ul></div>`;
    }

    const incoming = DATA.relations.filter((relation) => relation.to === module.id);
    const outgoing = DATA.relations.filter((relation) => relation.from === module.id);
    if (incoming.length || outgoing.length) {
      const rows = [
        ...incoming.map((relation) => relationRow(relation, 'in')),
        ...outgoing.map((relation) => relationRow(relation, 'out')),
      ].join('');
      html += `<div class="detail-section"><h2>Relations</h2><div class="relation-list">${rows}</div></div>`;
    }

    if (module.paths?.length) {
      html += `<div class="detail-section"><h2>Owned paths</h2><div class="path-list">${module.paths.map((path) => `<span class="path-chip">${esc(path)}</span>`).join('')}</div></div>`;
    }

    html += `<div class="detail-section"><h2>Source</h2>${sourceView(module)}</div>`;
    return html;
  }

  function relationRow(relation, direction) {
    const other = direction === 'out' ? relation.to : relation.from;
    return `<div class="relation"><span class="arrow">${direction === 'out' ? '→' : '←'}</span><button type="button" data-module="${esc(other)}">${esc(moduleTitle(other))}</button><span>${esc(relation.label || '')}</span></div>`;
  }

  function sourceView(module) {
    if (!module.sources?.length) {
      return '<div class="empty">Planned module — no source yet. The contract above is the source of truth.</div>';
    }

    const index = Math.min(state.sourceIndex, module.sources.length - 1);
    const source = module.sources[index];
    const tabs = module.sources.map((item, itemIndex) => `<button class="source-tab${itemIndex === index ? ' active' : ''}" type="button" data-source="${itemIndex}" title="${esc(item.path)}">${esc(item.path.split('/').pop())}</button>`).join('');
    return `<div class="source-tabs">${tabs}</div>${sourceBox(source)}`;
  }

  function sourceBox(source) {
    const lines = source.text.replace(/\n$/, '').split('\n');
    const body = lines.map((line, index) => `<span class="source-line"><span class="ln">${index + 1}</span><span class="lc">${esc(line) || ' '}</span></span>`).join('');
    return `<div class="source-box" role="region" aria-label="${esc(source.path)}">${body}</div>${source.truncated ? '<p class="hint">Truncated. Open the file in the repository for the full source.</p>' : ''}`;
  }

  function docDetails(doc) {
    let html = detailsHead(`${docKind(doc.kind)} · ${doc.id}`);
    html += `<h1 class="details-title">${esc(doc.title)}</h1>`;
    html += `<div class="badges">${doc.status ? `<span class="badge">${esc(doc.status)}</span>` : ''}<span class="badge">${esc(docKind(doc.kind))}</span></div>`;
    html += `<div class="md">${doc.body_html}</div>`;
    html += `<p class="hint">${esc(doc.path)}</p>`;
    return html;
  }

  function openCard(id) {
    const card = cardById.get(id);
    if (!card) return;
    state.view = 'board';
    state.milestone = card.milestone;
    state.selection = { type: 'card', id };
    state.query = '';
    search.value = '';
    render();
    syncHash();
  }

  function openModule(id) {
    if (!moduleById.has(id)) return;
    state.view = 'arch';
    state.selection = { type: 'module', id };
    state.sourceIndex = 0;
    state.query = '';
    search.value = '';
    render();
    syncHash();
  }

  function openDoc(id) {
    if (!docById.has(id)) return;
    state.view = 'docs';
    state.selection = { type: 'doc', id };
    state.query = '';
    search.value = '';
    render();
    syncHash();
  }

  function setView(view) {
    if (!VIEWS.includes(view)) return;
    state.view = view;
    state.selection = null;
    state.query = '';
    search.value = '';
    render();
    syncHash();
  }

  function setMilestone(id) {
    state.view = 'board';
    state.milestone = id;
    state.selection = null;
    state.query = '';
    search.value = '';
    render();
    syncHash();
  }

  function clearSelection() {
    state.selection = null;
    render();
    syncHash();
  }

  function syncHash() {
    let hash = `#/${state.view}`;
    if (state.selection) hash += `/${encodeURIComponent(state.selection.id)}`;
    if (location.hash === hash) return;
    try {
      history.replaceState(null, '', hash);
    } catch {
      location.hash = hash;
    }
  }

  function applyHash() {
    const raw = location.hash.replace(/^#\/?/, '');
    if (!raw) {
      state.view = 'board';
      state.selection = null;
      render();
      return;
    }

    const parts = raw.split('/');
    const view = parts[0];
    const id = parts.length > 1 ? decodeURIComponent(parts.slice(1).join('/')) : '';

    if (!VIEWS.includes(view)) {
      state.view = 'board';
      state.selection = null;
      render();
      return;
    }

    state.view = view;
    state.selection = null;

    if (id) {
      if (view === 'board' && cardById.has(id)) {
        state.selection = { type: 'card', id };
        state.milestone = cardById.get(id).milestone;
      } else if (view === 'arch' && moduleById.has(id)) {
        state.selection = { type: 'module', id };
        state.sourceIndex = 0;
      } else if (view === 'docs' && docById.has(id)) {
        state.selection = { type: 'doc', id };
      }
    }

    state.query = '';
    search.value = '';
    render();
  }

  function scheduleEdges() {
    cancelAnimationFrame(edgeFrame);
    edgeFrame = requestAnimationFrame(() => {
      edgeFrame = requestAnimationFrame(drawEdges);
    });
  }

  function drawEdges() {
    const arch = document.getElementById('arch-canvas');
    const svg = document.getElementById('arch-edges');
    if (!arch || !svg) return;

    const rect = arch.getBoundingClientRect();
    const width = Math.max(arch.scrollWidth, arch.clientWidth);
    const height = Math.max(arch.scrollHeight, arch.clientHeight);
    svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
    svg.setAttribute('width', width);
    svg.setAttribute('height', height);

    const defs = '<defs><marker id="ni-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path class="arrow-head" d="M 0 0 L 10 5 L 0 10 z"/></marker><marker id="ni-arrow-active" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path class="arrow-head-active" d="M 0 0 L 10 5 L 0 10 z"/></marker></defs>';
    const parts = [defs];
    const selectedModule = state.selection?.type === 'module' ? state.selection.id : null;

    for (const relation of DATA.relations) {
      const source = arch.querySelector(`[data-module="${cssEscape(relation.from)}"]`);
      const target = arch.querySelector(`[data-module="${cssEscape(relation.to)}"]`);
      if (!source || !target || source.classList.contains('dim') || target.classList.contains('dim')) continue;

      const a = source.getBoundingClientRect();
      const b = target.getBoundingClientRect();
      if (a.width === 0 || b.width === 0) continue;

      let x1;
      let y1;
      let x2;
      let y2;
      let path;
      let labelY;

      if (b.top > a.bottom + 6) {
        x1 = a.left + a.width / 2 - rect.left;
        y1 = a.bottom - rect.top;
        x2 = b.left + b.width / 2 - rect.left;
        y2 = b.top - rect.top;
        const midY = (y1 + y2) / 2;
        path = `M${x1},${y1} C${x1},${midY} ${x2},${midY} ${x2},${y2}`;
        labelY = y1 + 12;
      } else if (b.bottom < a.top - 6) {
        x1 = a.left + a.width / 2 - rect.left;
        y1 = a.top - rect.top;
        x2 = b.left + b.width / 2 - rect.left;
        y2 = b.bottom - rect.top;
        const midY = (y1 + y2) / 2;
        path = `M${x1},${y1} C${x1},${midY} ${x2},${midY} ${x2},${y2}`;
        labelY = y1 - 7;
      } else {
        const toRight = b.left >= a.left;
        x1 = (toRight ? a.right : a.left) - rect.left;
        y1 = a.top + a.height / 2 - rect.top;
        x2 = (toRight ? b.left : b.right) - rect.left;
        y2 = b.top + b.height / 2 - rect.top;
        const midX = (x1 + x2) / 2;
        path = `M${x1},${y1} C${midX},${y1} ${midX},${y2} ${x2},${y2}`;
        labelY = a.top - rect.top - 7;
      }

      const connected = selectedModule && (relation.from === selectedModule || relation.to === selectedModule);
      const className = connected ? 'edge edge-active' : 'edge';
      const marker = connected ? 'ni-arrow-active' : 'ni-arrow';
      const title = `${relation.from} → ${relation.to}${relation.label ? ` · ${relation.label}` : ''}`;
      parts.push(`<path class="${className}" d="${path}" marker-end="url(#${marker})"><title>${esc(title)}</title></path>`);

      if (connected && relation.label) {
        const lx = (x1 + x2) / 2;
        parts.push(`<text class="edge-label edge-label-active" x="${lx}" y="${labelY}" text-anchor="middle">${esc(relation.label)}</text>`);
      }
    }

    svg.innerHTML = parts.join('');
  }

  function cssEscape(value) {
    if (window.CSS?.escape) return CSS.escape(value);
    return String(value).replace(/["\\]/g, '\\$&');
  }

  function applyTheme(theme) {
    document.documentElement.dataset.theme = theme;
  }

  function bindEvents() {
    document.addEventListener('click', (event) => {
      if (!(event.target instanceof Element)) return;
      const target = event.target.closest('[data-view],[data-milestone],[data-card],[data-module],[data-doc],[data-source],[data-close]');
      if (!target) return;

      if (target.dataset.view) {
        event.preventDefault();
        setView(target.dataset.view);
        return;
      }
      if (target.dataset.milestone) {
        setMilestone(target.dataset.milestone);
        return;
      }
      if (target.dataset.card) {
        openCard(target.dataset.card);
        return;
      }
      if (target.dataset.module) {
        openModule(target.dataset.module);
        return;
      }
      if (target.dataset.doc) {
        openDoc(target.dataset.doc);
        return;
      }
      if (target.hasAttribute('data-source')) {
        state.sourceIndex = Number(target.dataset.source) || 0;
        renderDetails();
        return;
      }
      if (target.hasAttribute('data-close')) {
        clearSelection();
      }
    });

    search.addEventListener('input', () => {
      state.query = search.value.trim().toLowerCase();
      renderCanvas();
    });

    themeButton.addEventListener('click', () => {
      const next = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark';
      applyTheme(next);
      try {
        localStorage.setItem('ni-theme', next);
      } catch {
        /* file:// or private mode */
      }
      scheduleEdges();
    });

    document.addEventListener('keydown', (event) => {
      const typing = document.activeElement instanceof HTMLInputElement || document.activeElement instanceof HTMLTextAreaElement;

      if (event.key === '/' && !typing) {
        event.preventDefault();
        search.focus();
        search.select();
        return;
      }

      if (event.key === 'Escape') {
        if (document.activeElement === search) {
          search.value = '';
          state.query = '';
          search.blur();
          renderCanvas();
        } else if (state.selection) {
          clearSelection();
        }
        return;
      }

      if (!typing && ['1', '2', '3', '4'].includes(event.key)) {
        setView(VIEWS[Number(event.key) - 1]);
      }
    });

    window.addEventListener('hashchange', applyHash);
    window.addEventListener('resize', scheduleEdges);

    if (document.fonts?.ready) {
      document.fonts.ready.then(scheduleEdges).catch(() => {});
    }
  }

  function render() {
    renderTabs();
    renderChrome();
    renderRail();
    renderCanvas();
    renderDetails();
  }

  function init() {
    const saved = (() => {
      try {
        return localStorage.getItem('ni-theme');
      } catch {
        return null;
      }
    })();
    const preferred = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    applyTheme(saved || preferred);
    bindEvents();

    if (location.hash) {
      applyHash();
    } else {
      render();
      syncHash();
    }
  }

  init();
})();
