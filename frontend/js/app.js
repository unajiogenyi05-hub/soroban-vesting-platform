'use strict';

// ── Config ────────────────────────────────────────────────────────────────────
const API_BASE = window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1'
  ? 'http://localhost:3001/api'
  : '/api';

// ── State ─────────────────────────────────────────────────────────────────────
const state = {
  vestingContractId: '',
  tokenContractId: '',
  multisigContractId: '',
};

// ── Utils ─────────────────────────────────────────────────────────────────────

/**
 * Fetch wrapper that always returns { ok, data, error }.
 */
async function apiFetch(path, options = {}) {
  try {
    const res = await fetch(`${API_BASE}${path}`, {
      headers: { 'Content-Type': 'application/json' },
      ...options,
    });
    const json = await res.json();
    return { ok: res.ok, data: json.data, error: json.error };
  } catch (err) {
    return { ok: false, data: null, error: err.message };
  }
}

/**
 * Show a toast notification.
 */
function showToast(message, type = 'info') {
  const container = document.getElementById('toastContainer');
  const toast = document.createElement('div');
  toast.className = `toast toast--${type}`;
  toast.textContent = message;
  toast.setAttribute('role', 'alert');
  container.appendChild(toast);
  setTimeout(() => {
    toast.style.opacity = '0';
    toast.style.transform = 'translateY(16px)';
    toast.style.transition = 'all 0.3s ease';
    setTimeout(() => toast.remove(), 320);
  }, 3500);
}

/**
 * Show result in a form result div.
 */
function showResult(id, message, type = 'success') {
  const el = document.getElementById(id);
  if (!el) return;
  el.textContent = message;
  el.className = `form-result ${type}`;
}

/**
 * Clear a result div.
 */
function clearResult(id) {
  const el = document.getElementById(id);
  if (el) { el.textContent = ''; el.className = 'form-result'; }
}

// ── Tab navigation ────────────────────────────────────────────────────────────

document.querySelectorAll('.nav-btn').forEach((btn) => {
  btn.addEventListener('click', () => {
    const tab = btn.dataset.tab;

    document.querySelectorAll('.nav-btn').forEach((b) => {
      b.classList.remove('active');
      b.setAttribute('aria-selected', 'false');
    });
    btn.classList.add('active');
    btn.setAttribute('aria-selected', 'true');

    document.querySelectorAll('.tab-panel').forEach((panel) => {
      panel.classList.toggle('hidden', panel.id !== `tab-${tab}`);
    });
  });
});

// ── Health check ──────────────────────────────────────────────────────────────

async function checkApiHealth() {
  const dot = document.getElementById('apiStatusDot');
  const text = document.getElementById('apiStatusText');
  const statApi = document.getElementById('statApi');

  const { ok, data } = await apiFetch('/health');
  if (ok) {
    dot.className = 'status-dot ok';
    text.textContent = `Connected (${data?.network || 'testnet'})`;
    if (statApi) statApi.textContent = '✓ Online';
  } else {
    dot.className = 'status-dot error';
    text.textContent = 'API offline';
    if (statApi) statApi.textContent = '✗ Offline';
  }
}

// ── Dashboard ─────────────────────────────────────────────────────────────────

async function loadDashboardStats() {
  const [vestCount, tokenInfo, propCount] = await Promise.all([
    apiFetch(`/vesting/count?contractId=${state.vestingContractId}`),
    apiFetch(`/token/info?contractId=${state.tokenContractId}`),
    apiFetch(`/multisig/count?contractId=${state.multisigContractId}`),
  ]);

  const statSchedules = document.getElementById('statSchedules');
  const statSupply    = document.getElementById('statSupply');
  const statProposals = document.getElementById('statProposals');

  if (statSchedules) statSchedules.textContent = vestCount.ok ? vestCount.data?.count ?? '0' : '—';
  if (statSupply)    statSupply.textContent    = tokenInfo.ok  ? tokenInfo.data?.totalSupply ?? '0' : '—';
  if (statProposals) statProposals.textContent = propCount.ok  ? propCount.data?.count ?? '0' : '—';
}

// Save addresses
document.getElementById('saveAddresses')?.addEventListener('click', () => {
  state.vestingContractId  = document.getElementById('vestingContractId').value.trim();
  state.tokenContractId    = document.getElementById('tokenContractId').value.trim();
  state.multisigContractId = document.getElementById('multisigContractId').value.trim();
  loadDashboardStats();
  showToast('Contract addresses saved', 'info');
});

// ── Vesting tab ───────────────────────────────────────────────────────────────

document.getElementById('createScheduleForm')?.addEventListener('submit', async (e) => {
  e.preventDefault();
  clearResult('createScheduleResult');

  const body = {
    contractId:    state.vestingContractId,
    from:          document.getElementById('cs-from').value.trim(),
    beneficiary:   document.getElementById('cs-beneficiary').value.trim(),
    tokenAddress:  document.getElementById('cs-token').value.trim(),
    totalAmount:   Number(document.getElementById('cs-amount').value),
    cliffDuration: Number(document.getElementById('cs-cliff').value),
    totalDuration: Number(document.getElementById('cs-duration').value),
  };

  const { ok, data, error } = await apiFetch('/vesting/schedule', {
    method: 'POST',
    body: JSON.stringify(body),
  });

  if (ok) {
    showResult('createScheduleResult', `✓ ${data?.message || 'Schedule queued'}`, 'success');
    showToast('Schedule creation queued', 'success');
  } else {
    showResult('createScheduleResult', `✗ ${error || 'Request failed'}`, 'error');
    showToast(error || 'Request failed', 'error');
  }
});

document.getElementById('checkClaimableBtn')?.addEventListener('click', async () => {
  clearResult('claimResult');
  const id = document.getElementById('claim-id').value.trim();
  if (!id) { showResult('claimResult', '✗ Enter a schedule ID', 'error'); return; }

  const { ok, data, error } = await apiFetch(
    `/vesting/claimable/${id}?contractId=${state.vestingContractId}`
  );
  if (ok) {
    showResult('claimResult', `Claimable: ${data?.claimableAmount ?? 0} tokens`, 'success');
  } else {
    showResult('claimResult', `✗ ${error}`, 'error');
  }
});

document.getElementById('claimForm')?.addEventListener('submit', async (e) => {
  e.preventDefault();
  clearResult('claimResult');

  const { ok, data, error } = await apiFetch('/vesting/claim', {
    method: 'POST',
    body: JSON.stringify({
      contractId:  state.vestingContractId,
      scheduleId:  Number(document.getElementById('claim-id').value),
      beneficiary: document.getElementById('claim-beneficiary').value.trim(),
    }),
  });

  if (ok) {
    showResult('claimResult', `✓ ${data?.message || 'Claim queued'}`, 'success');
    showToast('Claim transaction ready', 'success');
  } else {
    showResult('claimResult', `✗ ${error}`, 'error');
  }
});

document.getElementById('revokeForm')?.addEventListener('submit', async (e) => {
  e.preventDefault();
  clearResult('revokeResult');

  const { ok, data, error } = await apiFetch('/vesting/revoke', {
    method: 'POST',
    body: JSON.stringify({
      contractId: state.vestingContractId,
      scheduleId: Number(document.getElementById('revoke-id').value),
      recipient:  document.getElementById('revoke-recipient').value.trim(),
    }),
  });

  if (ok) {
    showResult('revokeResult', `✓ ${data?.message || 'Revoke queued'}`, 'success');
    showToast('Revoke transaction ready', 'success');
  } else {
    showResult('revokeResult', `✗ ${error}`, 'error');
  }
});

// ── Token tab ─────────────────────────────────────────────────────────────────

document.getElementById('loadTokenInfoBtn')?.addEventListener('click', async () => {
  const { ok, data, error } = await apiFetch(
    `/token/info?contractId=${state.tokenContractId}`
  );
  const grid = document.getElementById('tokenInfoGrid');
  if (!grid) return;
  if (!ok) { grid.innerHTML = `<span style="color:var(--color-danger)">${error}</span>`; return; }

  const items = [
    ['Name',    data?.name        ?? '—'],
    ['Symbol',  data?.symbol      ?? '—'],
    ['Decimals',data?.decimals    ?? '—'],
    ['Supply',  data?.totalSupply ?? '—'],
  ];
  grid.innerHTML = items.map(([k, v]) =>
    `<div class="info-item">
       <span class="info-item__key">${k}</span>
       <span class="info-item__value">${v}</span>
     </div>`
  ).join('');
});

document.getElementById('balanceForm')?.addEventListener('submit', async (e) => {
  e.preventDefault();
  clearResult('balanceResult');
  const addr = document.getElementById('bal-addr').value.trim();
  const { ok, data, error } = await apiFetch(
    `/token/balance/${addr}?contractId=${state.tokenContractId}`
  );
  if (ok) {
    showResult('balanceResult', `Balance: ${data?.balance ?? 0}`, 'success');
  } else {
    showResult('balanceResult', `✗ ${error}`, 'error');
  }
});

document.getElementById('mintForm')?.addEventListener('submit', async (e) => {
  e.preventDefault();
  clearResult('mintResult');
  const { ok, data, error } = await apiFetch('/token/mint', {
    method: 'POST',
    body: JSON.stringify({
      contractId: state.tokenContractId,
      to:         document.getElementById('mint-to').value.trim(),
      amount:     Number(document.getElementById('mint-amount').value),
    }),
  });
  if (ok) {
    showResult('mintResult', `✓ ${data?.message || 'Mint queued'}`, 'success');
    showToast('Mint transaction ready', 'success');
  } else {
    showResult('mintResult', `✗ ${error}`, 'error');
  }
});

document.getElementById('transferForm')?.addEventListener('submit', async (e) => {
  e.preventDefault();
  clearResult('transferResult');
  const { ok, data, error } = await apiFetch('/token/transfer', {
    method: 'POST',
    body: JSON.stringify({
      contractId: state.tokenContractId,
      from:       document.getElementById('tf-from').value.trim(),
      to:         document.getElementById('tf-to').value.trim(),
      amount:     Number(document.getElementById('tf-amount').value),
    }),
  });
  if (ok) {
    showResult('transferResult', `✓ ${data?.message || 'Transfer queued'}`, 'success');
    showToast('Transfer transaction ready', 'success');
  } else {
    showResult('transferResult', `✗ ${error}`, 'error');
  }
});

// ── Multisig tab ──────────────────────────────────────────────────────────────

document.getElementById('submitProposalForm')?.addEventListener('submit', async (e) => {
  e.preventDefault();
  clearResult('proposalResult');
  const { ok, data, error } = await apiFetch('/multisig/proposal', {
    method: 'POST',
    body: JSON.stringify({
      contractId:  state.multisigContractId,
      proposer:    document.getElementById('prop-proposer').value.trim(),
      description: document.getElementById('prop-desc').value.trim(),
    }),
  });
  if (ok) {
    showResult('proposalResult', `✓ ${data?.message || 'Proposal queued'}`, 'success');
    showToast('Proposal submitted', 'success');
  } else {
    showResult('proposalResult', `✗ ${error}`, 'error');
  }
});

async function multisigAction(endpoint, extraBody) {
  clearResult('confirmResult');
  const proposalId = Number(document.getElementById('conf-id').value);
  const owner      = document.getElementById('conf-owner').value.trim();
  if (!proposalId || !owner) {
    showResult('confirmResult', '✗ Enter proposal ID and owner address', 'error');
    return;
  }
  const { ok, data, error } = await apiFetch(`/multisig/${endpoint}`, {
    method: 'POST',
    body: JSON.stringify({ contractId: state.multisigContractId, proposalId, owner, ...extraBody }),
  });
  if (ok) {
    showResult('confirmResult', `✓ ${data?.message || 'Done'}`, 'success');
    showToast(`${endpoint} successful`, 'success');
  } else {
    showResult('confirmResult', `✗ ${error}`, 'error');
  }
}

document.getElementById('confirmBtn')?.addEventListener('click', () => multisigAction('confirm'));
document.getElementById('revokeConfirmBtn')?.addEventListener('click', () => multisigAction('revoke-confirm'));
document.getElementById('executeBtn')?.addEventListener('click', async () => {
  clearResult('confirmResult');
  const proposalId = Number(document.getElementById('conf-id').value);
  if (!proposalId) { showResult('confirmResult', '✗ Enter proposal ID', 'error'); return; }
  const { ok, data, error } = await apiFetch('/multisig/execute', {
    method: 'POST',
    body: JSON.stringify({ contractId: state.multisigContractId, proposalId }),
  });
  if (ok) {
    showResult('confirmResult', `✓ ${data?.message || 'Execute queued'}`, 'success');
    showToast('Execute transaction ready', 'success');
  } else {
    showResult('confirmResult', `✗ ${error}`, 'error');
  }
});

document.getElementById('loadOwnersBtn')?.addEventListener('click', async () => {
  const { ok, data, error } = await apiFetch(
    `/multisig/owners?contractId=${state.multisigContractId}`
  );
  const list = document.getElementById('ownersList');
  if (!list) return;
  if (!ok) { list.innerHTML = `<li style="color:var(--color-danger)">${error}</li>`; return; }
  const owners = data?.owners || [];
  if (!owners.length) {
    list.innerHTML = '<li>No owners found (set MULTISIG_CONTRACT_ID)</li>';
    return;
  }
  list.innerHTML = owners.map((o) => `<li>${o}</li>`).join('');
});

// ── Init ──────────────────────────────────────────────────────────────────────

(async function init() {
  await checkApiHealth();
  await loadDashboardStats();
  // Re-check health every 30s
  setInterval(checkApiHealth, 30_000);
})();
