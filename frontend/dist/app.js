const form = document.querySelector('#auth-form');
const authWrap = document.querySelector('.auth-wrap');
const heading = document.querySelector('.auth-heading');
const nameFields = document.querySelector('#name-fields');
const termsRow = document.querySelector('#terms-row');
const password = document.querySelector('#password');
const primaryButton = document.querySelector('.primary-button span');
const successState = document.querySelector('#success-state');
const resultsForm = document.querySelector('#results-form');
const resultsView = document.querySelector('#results-view');
const toast = document.querySelector('#toast');
const apiMeta = document.querySelector('meta[name="sphinx-api-base"]');
const apiOverride = new URLSearchParams(window.location.search).get('apiBase');
const API_BASE = (apiOverride || apiMeta?.content || window.location.origin).replace(/\/$/, '');
const SESSION_KEY = 'sphinx.auth.session';

let mode = 'login';
let session = restoreSession();

function setMode(nextMode) {
  mode = nextMode;
  const isSignup = mode === 'signup';
  authWrap.classList.toggle('signup-mode', isSignup);
  nameFields.classList.toggle('is-hidden', !isSignup);
  termsRow.classList.toggle('is-hidden', !isSignup);
  document.querySelector('.form-options').classList.toggle('is-hidden', isSignup);
  heading.querySelector('.kicker').textContent = isSignup ? 'JOIN THE CURIOUS' : 'WELCOME BACK';
  heading.querySelector('h2').innerHTML = isSignup ? 'Create your<br>Sphinx account.' : 'Pick up where<br>you left off.';
  heading.querySelector('p').innerHTML = isSignup
    ? 'Already have an account? <button class="text-link" type="button" data-switch="login">Log in</button>'
    : 'New to Sphinx? <button class="text-link" type="button" data-switch="signup">Create an account</button>';
  primaryButton.textContent = isSignup ? 'Create my account' : 'Log in to Sphinx';
  password.autocomplete = isSignup ? 'new-password' : 'current-password';
  document.querySelectorAll('.field').forEach((field) => field.classList.remove('has-error'));

  if (!window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
    authWrap.animate(
      [
        { opacity: 0.45, transform: 'translateY(8px)' },
        { opacity: 1, transform: 'translateY(0)' }
      ],
      { duration: 320, easing: 'cubic-bezier(.2,.75,.25,1)' }
    );
  }
}

document.addEventListener('click', (event) => {
  const switcher = event.target.closest('[data-switch]');
  if (switcher) setMode(switcher.dataset.switch);
});

document.querySelector('[data-password-toggle]').addEventListener('click', (event) => {
  const button = event.currentTarget;
  const showing = password.type === 'text';
  password.type = showing ? 'password' : 'text';
  button.setAttribute('aria-label', showing ? 'Show password' : 'Hide password');
});

function setError(input, message) {
  const field = input.closest('.field');
  field.classList.toggle('has-error', Boolean(message));
  field.querySelector('.error-message').textContent = message || '';
}

function validate() {
  let valid = true;
  const email = document.querySelector('#email');
  const emailOkay = /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email.value.trim());
  setError(email, emailOkay ? '' : 'Enter a valid email address');
  valid = valid && emailOkay;

  const passwordOkay = password.value.length >= (mode === 'signup' ? 8 : 1);
  setError(password, passwordOkay ? '' : mode === 'signup' ? 'Password must be at least 8 characters' : 'Enter your password');
  valid = valid && passwordOkay;

  if (mode === 'signup') {
    ['first-name', 'last-name'].forEach((id) => {
      const input = document.querySelector(`#${id}`);
      const okay = input.value.trim().length >= 2;
      setError(input, okay ? '' : 'Please enter your name');
      valid = valid && okay;
    });
    const terms = document.querySelector('[name="terms"]');
    if (!terms.checked) {
      showToast('Please agree to the terms to continue.');
      valid = false;
    }
  }
  return valid;
}

function errorMessage(payload, fallback) {
  if (typeof payload?.error === 'string') return payload.error;
  if (typeof payload?.error?.message === 'string') return payload.error.message;
  return fallback;
}

async function apiRequest(path, { method = 'GET', body, token } = {}) {
  const headers = { Accept: 'application/json' };
  if (body !== undefined) headers['Content-Type'] = 'application/json';
  if (token) headers.Authorization = `Bearer ${token}`;

  let response;
  try {
    response = await fetch(`${API_BASE}${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body)
    });
  } catch {
    throw new Error(`Cannot reach the Sphinx API at ${API_BASE}. Start the backend and try again.`);
  }

  const text = await response.text();
  let payload = null;
  if (text) {
    try {
      payload = JSON.parse(text);
    } catch {
      payload = null;
    }
  }

  if (!response.ok) {
    const error = new Error(errorMessage(payload, `Request failed with HTTP ${response.status}`));
    error.status = response.status;
    throw error;
  }
  return payload;
}

function restoreSession() {
  for (const storage of [window.sessionStorage, window.localStorage]) {
    try {
      const saved = JSON.parse(storage.getItem(SESSION_KEY));
      if (!saved?.token || !saved?.user || saved.expiresAt <= Date.now()) {
        storage.removeItem(SESSION_KEY);
        continue;
      }
      return saved;
    } catch {
      storage.removeItem(SESSION_KEY);
    }
  }
  return null;
}

function saveSession(login, remember) {
  window.localStorage.removeItem(SESSION_KEY);
  window.sessionStorage.removeItem(SESSION_KEY);
  const saved = {
    token: login.token,
    user: login.user,
    expiresAt: Date.now() + login.expires_in * 1000
  };
  (remember ? window.localStorage : window.sessionStorage).setItem(SESSION_KEY, JSON.stringify(saved));
  session = saved;
}

function clearSession() {
  window.localStorage.removeItem(SESSION_KEY);
  window.sessionStorage.removeItem(SESSION_KEY);
  session = null;
}

function setAuthBusy(busy) {
  const button = form.querySelector('.primary-button');
  button.disabled = busy;
  primaryButton.textContent = busy
    ? mode === 'signup' ? 'Creating account…' : 'Signing in…'
    : mode === 'signup' ? 'Create my account' : 'Log in to Sphinx';
}

async function authenticate() {
  const email = document.querySelector('#email').value.trim();
  const passwordValue = password.value;
  if (mode === 'signup') {
    const name = `${document.querySelector('#first-name').value.trim()} ${document.querySelector('#last-name').value.trim()}`;
    await apiRequest('/api/v1/auth/register', {
      method: 'POST',
      body: { name, email, password: passwordValue }
    });
  }
  return apiRequest('/api/v1/auth/login', {
    method: 'POST',
    body: { email, password: passwordValue }
  });
}

form.addEventListener('submit', async (event) => {
  event.preventDefault();
  if (!validate()) return;
  setAuthBusy(true);
  try {
    const login = await authenticate();
    saveSession(login, document.querySelector('[name="remember"]').checked);
    showAuthenticated();
    showToast(mode === 'signup' ? 'Account created and signed in.' : 'Signed in successfully.');
  } catch (error) {
    showToast(error.message);
  } finally {
    setAuthBusy(false);
  }
});

function showAuthenticated() {
  form.classList.add('is-hidden');
  heading.classList.add('is-hidden');
  authWrap.classList.add('results-mode');
  successState.classList.add('is-visible');
  document.querySelector('#signed-in-name').textContent = session.user.name.split(/\s+/)[0] || 'friend';
}

function showLogin() {
  successState.classList.remove('is-visible');
  resultsView.classList.add('is-hidden');
  authWrap.classList.remove('results-mode');
  form.classList.remove('is-hidden');
  heading.classList.remove('is-hidden');
  form.reset();
  setMode('login');
}

function formatPercentage(value) {
  return value == null ? '—' : `${Number(value).toFixed(1).replace(/\.0$/, '')}%`;
}

function resultRow(label, value, detail) {
  const row = document.createElement('div');
  row.className = 'result-row';
  const title = document.createElement('strong');
  title.textContent = label;
  const result = document.createElement('span');
  result.textContent = value;
  const trailing = document.createElement('span');
  trailing.className = 'result-detail';
  trailing.textContent = detail;
  row.append(title, result, trailing);
  return row;
}

function renderList(container, rows, emptyMessage) {
  container.replaceChildren();
  if (!rows.length) {
    const empty = document.createElement('p');
    empty.className = 'result-empty';
    empty.textContent = emptyMessage;
    container.append(empty);
    return;
  }
  container.append(...rows);
}

function renderResults(results) {
  document.querySelector('#best-score').textContent = results.best_score == null ? '—' : `${results.best_score} pts`;
  document.querySelector('#best-percentage').textContent = formatPercentage(results.best_percentage);
  document.querySelector('#latest-rank').textContent = results.rank == null ? '—' : `#${results.rank}`;
  document.querySelector('#latest-revision').textContent = results.revision_number == null
    ? 'No attempts'
    : `Latest revision ${results.revision_number}`;

  const revisions = [...(results.revision_results || [])]
    .sort((left, right) => left.quiz_revision_number - right.quiz_revision_number)
    .map((revision) => resultRow(
      `Revision ${revision.quiz_revision_number}`,
      formatPercentage(revision.best_percentage),
      `Rank #${revision.rank}`
    ));
  renderList(document.querySelector('#revision-results'), revisions, 'No revision rankings are available yet.');

  const attempts = [...(results.attempts || [])]
    .sort((left, right) => new Date(right.submitted_at) - new Date(left.submitted_at))
    .map((attempt) => resultRow(
      `Attempt ${attempt.attempt_number}`,
      `${attempt.score}/${attempt.total_questions} · ${formatPercentage(attempt.percentage)}`,
      new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(attempt.submitted_at))
    ));
  renderList(document.querySelector('#attempt-results'), attempts, 'You have not submitted an attempt for this quiz.');
  resultsView.classList.remove('is-hidden');
}

resultsForm.addEventListener('submit', async (event) => {
  event.preventDefault();
  const quizInput = document.querySelector('#results-quiz-id');
  const quizId = quizInput.value.trim();
  const validUuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(quizId);
  setError(quizInput, validUuid ? '' : 'Enter a valid quiz UUID');
  if (!validUuid) return;

  const button = resultsForm.querySelector('button');
  const label = button.querySelector('span');
  button.disabled = true;
  label.textContent = 'Loading…';
  try {
    const results = await apiRequest(`/api/v1/quizzes/${encodeURIComponent(quizId)}/my-results`, { token: session.token });
    renderResults(results);
  } catch (error) {
    if (error.status === 401) {
      clearSession();
      showLogin();
      showToast('Your session expired. Please log in again.');
    } else {
      showToast(error.message);
    }
  } finally {
    button.disabled = false;
    label.textContent = 'Load my results';
  }
});

document.querySelector('#logout-button').addEventListener('click', async (event) => {
  const button = event.currentTarget;
  button.disabled = true;
  try {
    await apiRequest('/api/v1/auth/logout', { method: 'POST', token: session?.token });
  } catch (error) {
    if (error.status !== 401) showToast(error.message);
  } finally {
    clearSession();
    showLogin();
    button.disabled = false;
  }
});

document.querySelectorAll('input').forEach((input) => {
  input.addEventListener('input', () => {
    if (input.closest('.field')) setError(input, '');
  });
});

document.querySelectorAll('[data-provider]').forEach((button) => {
  button.addEventListener('click', () => showToast(`${button.dataset.provider} sign-in is not configured yet.`));
});

document.querySelector('.forgot-link').addEventListener('click', () => showToast('Password recovery is not configured yet.'));

let toastTimer;
function showToast(message) {
  toast.textContent = message;
  toast.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove('show'), 4200);
}

if (session) showAuthenticated();
