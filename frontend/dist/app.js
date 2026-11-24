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
const DEMO_KEY = 'sphinx.demo.workspace';

const apiOwners = [
  {
    name: 'Gangadhar', area: 'Authentication, teams and PostgreSQL foundation', endpoints: [
      ['GET', '/health/live', /^\/health\/live$/, 'Health check', 'Confirms that the Axum service is running.'],
      ['POST', '/api/v1/auth/register', /^\/api\/v1\/auth\/register$/, 'Register account', 'Validates and stores a new user account.'],
      ['POST', '/api/v1/auth/login', /^\/api\/v1\/auth\/login$/, 'Login', 'Verifies credentials and creates an expiring session token.'],
      ['GET', '/api/v1/auth/me', /^\/api\/v1\/auth\/me$/, 'Current user', 'Returns the user identified by the Bearer token.'],
      ['POST', '/api/v1/auth/logout', /^\/api\/v1\/auth\/logout$/, 'Logout', 'Revokes the current login session.'],
      ['POST', '/api/v1/teams', /^\/api\/v1\/teams\/?$/, 'Create team', 'Creates a team and makes its creator the administrator.'],
      ['GET', '/api/v1/teams', /^\/api\/v1\/teams\/?$/, 'List teams', 'Lists teams and roles for the authenticated user.'],
      ['POST', '/api/v1/teams/{team_id}/members', /^\/api\/v1\/teams\/[^/]+\/members$/, 'Add member', 'Adds an existing user to a team after an admin check.'],
      ['GET', '/api/v1/teams/{team_id}/members', /^\/api\/v1\/teams\/[^/]+\/members$/, 'List members', 'Returns team members and their roles.']
    ]
  },
  {
    name: 'Ashish', area: 'Quiz lifecycle, validation and revisions', endpoints: [
      ['POST', '/questions/validate', /^\/questions\/validate$/, 'Validate question', 'Runs the Rust question business rules without writing to the database.'],
      ['POST', '/api/v1/teams/{team_id}/quizzes', /^\/api\/v1\/teams\/[^/]+\/quizzes$/, 'Create draft quiz', 'Admin-only creation of a saved, unpublished quiz.'],
      ['POST', '/api/v1/teams/{team_id}/quizzes/{quiz_id}/questions', /^\/api\/v1\/teams\/[^/]+\/quizzes\/[^/]+\/questions$/, 'Add question', 'Validates and stores a question in the draft.'],
      ['PUT', '/api/v1/teams/{team_id}/quizzes/{quiz_id}/questions/{question_id}', /^\/api\/v1\/teams\/[^/]+\/quizzes\/[^/]+\/questions\/[^/]+$/, 'Edit or revise question', 'Updates a draft or creates an immutable revision after publication.'],
      ['PUT', '/api/v1/quizzes/{quiz_id}/eligibilities', /^\/api\/v1\/quizzes\/[^/]+\/eligibilities$/, 'Assign participants', 'Verifies and saves the selected team members for a draft quiz.'],
      ['POST', '/api/v1/quizzes/{quiz_id}/publish', /^\/api\/v1\/quizzes\/[^/]+\/publish$/, 'Publish quiz', 'Atomically creates a revision, question versions and the open time window.'],
      ['POST', '/api/v1/quizzes/{quiz_id}/close', /^\/api\/v1\/quizzes\/[^/]+\/close$/, 'Close quiz', 'Admin-only early closure using PostgreSQL time and locks.']
    ]
  },
  {
    name: 'Abhishek', area: 'Participation, scoring, review and analytics API', endpoints: [
      ['GET', '/api/v1/teams/{team_id}/quizzes/available', /^\/api\/v1\/teams\/[^/]+\/quizzes\/available$/, 'Available quizzes', 'Lists open eligible quizzes with attempts remaining.'],
      ['POST', '/api/v1/quizzes/{quiz_id}/attempt-sessions', /^\/api\/v1\/quizzes\/[^/]+\/attempt-sessions$/, 'Start attempt', 'Checks eligibility and pins the member to a quiz revision.'],
      ['GET', '/api/v1/attempt-sessions/{session_id}/questions', /^\/api\/v1\/attempt-sessions\/[^/]+\/questions$/, 'Session questions', 'Returns public questions without answer keys.'],
      ['POST', '/api/v1/attempt-sessions/{session_id}/submit', /^\/api\/v1\/attempt-sessions\/[^/]+\/submit$/, 'Submit answers', 'Validates, scores and atomically saves one attempt.'],
      ['GET', '/api/v1/quizzes/{quiz_id}/my-attempts', /^\/api\/v1\/quizzes\/[^/]+\/my-attempts$/, 'Attempt history', 'Returns only the authenticated member’s attempts.'],
      ['GET', '/api/v1/attempts/{attempt_id}/review', /^\/api\/v1\/attempts\/[^/]+\/review$/, 'Attempt review', 'Returns post-submission answers and explanations to an authorized user.'],
      ['GET', '/api/v1/quizzes/{quiz_id}/analytics', /^\/api\/v1\/quizzes\/[^/]+\/analytics$/, 'Quiz analytics', 'Returns team-wide participation and rankings to an administrator.']
    ]
  },
  {
    name: 'Sushma', area: 'Personal insights and revision-aware results', endpoints: [
      ['GET', '/api/v1/quizzes/{quiz_id}/my-results', /^\/api\/v1\/quizzes\/[^/]+\/my-results$/, 'My results', 'Returns personal attempts, latest-revision summary and revision-specific rank.']
    ]
  }
];

const apiStates = new Map();
apiOwners.forEach((owner) => owner.endpoints.forEach((endpoint, index) => {
  endpoint.key = `${owner.name}-${index}`;
  apiStates.set(endpoint.key, { state: 'idle', status: null, time: null });
}));

const preparedQuestions = [
  {
    question_text: 'What does PostgreSQL provide during quiz publication?',
    options: ['Atomic transactions', 'Browser animations', 'Email delivery'],
    correct_option_index: 0,
    explanation: 'The publication workflow uses a transaction so every database change succeeds or rolls back together.'
  },
  {
    question_text: 'Why does Sphinx create a new revision after a published question changes?',
    options: ['To preserve earlier attempts', 'To delete old results', 'To bypass authorization'],
    correct_option_index: 0,
    explanation: 'Immutable revisions keep earlier attempts tied to the exact questions and answer key participants saw.'
  },
  {
    question_text: 'Which layer identifies the signed-in user?',
    options: ['CSS', 'Session authentication', 'Quiz ranking'],
    correct_option_index: 1,
    explanation: 'The shared session extractor validates the Bearer token and supplies the authenticated user ID.'
  }
];

let demoWorkspace = null;
let activeAttempt = null;
let teamMemberships = [];
let selectedTeam = null;
let teamMembers = [];
let teamContextRequest = 0;
let availableQuizzesRequest = 0;
let teamMembersRequest = 0;

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
  if (typeof payload === 'string' && payload.trim()) return payload.trim();
  if (typeof payload?.error === 'string') return payload.error;
  if (typeof payload?.error?.message === 'string') return payload.error.message;
  return fallback;
}

function findApiEndpoint(method, path) {
  const pathname = path.split('?')[0];
  for (const owner of apiOwners) {
    for (const endpoint of owner.endpoints) {
      if (endpoint[0] === method && endpoint[2].test(pathname)) return endpoint;
    }
  }
  return null;
}

function setApiState(endpoint, state, status = null) {
  if (!endpoint) return;
  apiStates.set(endpoint.key, { state, status, time: new Date() });
  updateApiMonitor();
}

function apiStatusLabel(record) {
  if (record.state === 'pending') return 'Running';
  if (record.state === 'success') return `${record.status} OK`;
  if (record.state === 'error') return record.status ? `${record.status} Failed` : 'Network error';
  return 'Not used';
}

function renderApiMonitor() {
  const container = document.querySelector('#api-monitor-groups');
  container.replaceChildren();
  apiOwners.forEach((owner) => {
    const section = document.createElement('section');
    section.className = 'api-owner-group';
    const heading = document.createElement('div');
    heading.className = 'api-owner-heading';
    const identity = document.createElement('div');
    const name = document.createElement('strong');
    name.textContent = owner.name;
    const area = document.createElement('small');
    area.textContent = owner.area;
    identity.append(name, area);
    const count = document.createElement('span');
    count.textContent = `${owner.endpoints.length} APIs`;
    heading.append(identity, count);
    section.append(heading);

    owner.endpoints.forEach((endpoint) => {
      const [method, path, , title, description] = endpoint;
      const details = document.createElement('details');
      details.className = 'api-endpoint';
      details.dataset.apiKey = endpoint.key;
      details.dataset.state = 'idle';
      const summary = document.createElement('summary');
      const methodBadge = document.createElement('span');
      methodBadge.className = 'api-method';
      methodBadge.textContent = method;
      const route = document.createElement('span');
      route.className = 'api-path';
      route.textContent = path;
      const status = document.createElement('span');
      status.className = 'api-status';
      status.innerHTML = '<i class="api-dot"></i><span>Not used</span>';
      summary.append(methodBadge, route, status);
      const body = document.createElement('div');
      body.className = 'api-endpoint-body';
      const purpose = document.createElement('p');
      const bold = document.createElement('b');
      bold.textContent = `${title}: `;
      purpose.append(bold, description);
      const activity = document.createElement('p');
      activity.className = 'api-last-activity';
      activity.textContent = 'Not called during this browser session.';
      body.append(purpose, activity);
      details.append(summary, body);
      section.append(details);
    });
    container.append(section);
  });
  updateApiMonitor();
}

function updateApiMonitor() {
  const records = [...apiStates.values()];
  const used = records.filter((record) => record.state !== 'idle').length;
  document.querySelector('#api-monitor-count').textContent = `${used}/${records.length}`;
  document.querySelector('#api-success-count').textContent = records.filter((record) => record.state === 'success').length;
  document.querySelector('#api-pending-count').textContent = records.filter((record) => record.state === 'pending').length;
  document.querySelector('#api-error-count').textContent = records.filter((record) => record.state === 'error').length;
  apiStates.forEach((record, key) => {
    const row = document.querySelector(`[data-api-key="${key}"]`);
    if (!row) return;
    row.dataset.state = record.state;
    const dot = row.querySelector('.api-dot');
    dot.className = `api-dot${record.state === 'idle' ? '' : ` api-dot-${record.state}`}`;
    row.querySelector('.api-status span').textContent = apiStatusLabel(record);
    row.querySelector('.api-last-activity').textContent = record.time
      ? `Last called at ${new Intl.DateTimeFormat(undefined, { timeStyle: 'medium' }).format(record.time)}.`
      : 'Not called during this browser session.';
  });
}

function setApiMonitorOpen(open) {
  const panel = document.querySelector('#api-monitor');
  const toggle = document.querySelector('#api-monitor-toggle');
  panel.classList.toggle('is-open', open);
  panel.setAttribute('aria-hidden', String(!open));
  toggle.setAttribute('aria-expanded', String(open));
  document.querySelector('#api-monitor-backdrop').classList.toggle('is-hidden', !open);
}

document.querySelector('#api-monitor-toggle').addEventListener('click', () => setApiMonitorOpen(true));
document.querySelector('#api-monitor-close').addEventListener('click', () => setApiMonitorOpen(false));
document.querySelector('#api-monitor-backdrop').addEventListener('click', () => setApiMonitorOpen(false));
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') setApiMonitorOpen(false);
});

renderApiMonitor();

async function apiRequest(path, { method = 'GET', body, token } = {}) {
  const endpoint = findApiEndpoint(method, path);
  setApiState(endpoint, 'pending');
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
    setApiState(endpoint, 'error');
    throw new Error(`Cannot reach the Sphinx API at ${API_BASE}. Start the backend and try again.`);
  }

  setApiState(endpoint, response.ok ? 'success' : 'error', response.status);

  const text = await response.text();
  let payload = null;
  if (text) {
    try {
      payload = JSON.parse(text);
    } catch {
      payload = text;
    }
  }

  if (!response.ok) {
    const error = new Error(errorMessage(payload, `Request failed with HTTP ${response.status}`));
    error.status = response.status;
    if (response.status === 401 && token && session?.token === token) {
      clearSession();
      showLogin();
      error.message = 'Your session expired. Please log in again.';
      showToast(error.message);
    }
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
  document.querySelector('#signed-in-user-id').textContent = session.user.id;
  demoWorkspace = restoreDemoWorkspace();
  renderPreparedQuestions();
  restoreDemoControls();
  switchWorkspace('studio');
  loadTeamContext();
}

async function loadTeamContext(preferredTeamId = demoWorkspace?.teamId) {
  const requestId = ++teamContextRequest;
  const studioTab = document.querySelector('[data-workspace="studio"]');
  const select = document.querySelector('#team-context-select');
  select.disabled = true;
  try {
    const response = await apiRequest('/api/v1/teams', { token: session.token });
    if (requestId !== teamContextRequest || !session) return;
    teamMemberships = Array.isArray(response) ? response : [];
    select.replaceChildren();
    if (!teamMemberships.length) {
      select.append(new Option('No team yet — create one in Admin studio', ''));
      selectedTeam = null;
      studioTab.classList.remove('is-hidden');
      document.querySelector('#team-role-label').textContent = 'No membership';
      renderAvailableQuizzes([]);
      return;
    }
    teamMemberships.forEach((team) => select.append(new Option(`${team.name} · ${team.role}`, team.id)));
    const storedTeamId = window.localStorage.getItem(selectedTeamStorageKey());
    select.value = teamMemberships.some((team) => team.id === preferredTeamId)
      ? preferredTeamId
      : teamMemberships.some((team) => team.id === storedTeamId) ? storedTeamId : teamMemberships[0].id;
    selectedTeam = teamMemberships.find((team) => team.id === select.value) || null;
    applySelectedTeamRole();
    if (selectedTeam?.role !== 'admin') {
      switchWorkspace('play');
      showToast('Member access: take eligible quizzes and view your results.');
    }
  } catch (error) {
    if (requestId !== teamContextRequest) return;
    showToast(`Could not load team roles: ${error.message}`);
  } finally {
    if (requestId === teamContextRequest) select.disabled = false;
  }
}

function applySelectedTeamRole() {
  const studioTab = document.querySelector('[data-workspace="studio"]');
  const isAdmin = selectedTeam?.role === 'admin';
  studioTab.classList.toggle('is-hidden', !isAdmin);
  document.querySelector('#team-role-label').textContent = selectedTeam
    ? selectedTeam.role === 'admin' ? 'Team administrator' : 'Team member'
    : 'No membership';
  if (!isAdmin && document.querySelector('#studio-workspace').classList.contains('is-hidden') === false) {
    switchWorkspace('play');
  }
  if (selectedTeam) {
    window.localStorage.setItem(selectedTeamStorageKey(), selectedTeam.id);
    loadAvailableQuizzes(selectedTeam.id);
    if (isAdmin) loadTeamMembers(selectedTeam.id);
    else renderTeamMembers([]);
  } else {
    renderAvailableQuizzes([]);
    renderTeamMembers([]);
  }
}

async function loadTeamMembers(teamId) {
  const requestId = ++teamMembersRequest;
  document.querySelector('#participant-copy').textContent = 'Loading team members…';
  try {
    const response = await apiRequest(`/api/v1/teams/${teamId}/members`, { token: session.token });
    if (requestId !== teamMembersRequest || selectedTeam?.id !== teamId) return;
    teamMembers = (Array.isArray(response) ? response : []).filter((member) => member.role === 'member');
    renderTeamMembers(teamMembers);
  } catch (error) {
    if (requestId !== teamMembersRequest) return;
    teamMembers = [];
    renderTeamMembers([]);
    document.querySelector('#participant-copy').textContent = error.message;
  }
}

function renderTeamMembers(members) {
  const container = document.querySelector('#participant-grid');
  container.replaceChildren();
  if (!members.length) {
    const empty = document.createElement('p');
    empty.className = 'result-empty';
    empty.textContent = 'No members are available. Run the demo seed before presenting.';
    container.append(empty);
    document.querySelector('#participant-copy').textContent = '0 members selected';
    return;
  }
  members.forEach((member) => {
    const label = document.createElement('label');
    label.className = 'participant-option';
    const input = document.createElement('input');
    input.type = 'checkbox';
    input.value = member.user_id;
    input.checked = true;
    input.addEventListener('change', updateParticipantCount);
    const initials = document.createElement('span');
    initials.className = 'participant-avatar';
    initials.textContent = member.name.split(/\s+/).map((part) => part[0]).join('').slice(0, 2).toUpperCase();
    const copy = document.createElement('span');
    const name = document.createElement('strong');
    name.textContent = member.name;
    const role = document.createElement('small');
    role.textContent = 'Team member';
    copy.append(name, role);
    label.append(input, initials, copy);
    container.append(label);
  });
  updateParticipantCount();
}

function selectedParticipantIds() {
  return [...document.querySelectorAll('#participant-grid input:checked')].map((input) => input.value);
}

function updateParticipantCount() {
  const selected = selectedParticipantIds().length;
  document.querySelector('#participant-copy').textContent = `${selected} of ${teamMembers.length} members selected`;
}

document.querySelector('#select-all-participants').addEventListener('click', () => {
  document.querySelectorAll('#participant-grid input').forEach((input) => { input.checked = true; });
  updateParticipantCount();
});

document.querySelector('#team-context-select').addEventListener('change', (event) => {
  selectedTeam = teamMemberships.find((team) => team.id === event.target.value) || null;
  applySelectedTeamRole();
});

document.querySelector('#refresh-teams').addEventListener('click', () => loadTeamContext(selectedTeam?.id));

async function loadAvailableQuizzes(teamId) {
  const requestId = ++availableQuizzesRequest;
  const copy = document.querySelector('#available-copy');
  copy.textContent = 'Loading eligible quizzes…';
  try {
    const response = await apiRequest(`/api/v1/teams/${teamId}/quizzes/available`, { token: session.token });
    if (requestId !== availableQuizzesRequest || selectedTeam?.id !== teamId) return;
    const quizzes = Array.isArray(response?.quizzes) ? response.quizzes : [];
    renderAvailableQuizzes(quizzes);
    copy.textContent = quizzes.length
      ? 'Choose an open quiz from this team.'
      : 'No eligible open quizzes are available for this team.';
  } catch (error) {
    if (requestId !== availableQuizzesRequest) return;
    renderAvailableQuizzes([]);
    copy.textContent = error.message;
  }
}

function renderAvailableQuizzes(quizzes) {
  const container = document.querySelector('#available-quizzes');
  container.replaceChildren();
  quizzes.forEach((quiz) => {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'available-quiz';
    const title = document.createElement('strong');
    title.textContent = quiz.title;
    const detail = document.createElement('small');
    detail.textContent = `Open until ${formatDateTime(quiz.ends_at, { timeStyle: 'short' })}`;
    button.append(title, detail);
    button.addEventListener('click', () => {
      document.querySelector('#play-quiz-id').value = quiz.id;
      updateStartAvailability();
      document.querySelector('#play-availability').textContent = 'Eligible · open';
    });
    container.append(button);
  });
}

document.querySelector('#copy-user-id').addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(session.user.id);
    showToast('User ID copied. Give it to the team admin before publication.');
  } catch {
    showToast(`Copy this user ID: ${session.user.id}`);
  }
});

function showLogin() {
  teamContextRequest += 1;
  availableQuizzesRequest += 1;
  teamMembersRequest += 1;
  teamMemberships = [];
  selectedTeam = null;
  teamMembers = [];
  successState.classList.remove('is-visible');
  resultsView.classList.add('is-hidden');
  authWrap.classList.remove('results-mode');
  form.classList.remove('is-hidden');
  heading.classList.remove('is-hidden');
  form.reset();
  setMode('login');
  activeAttempt = null;
  document.querySelector('#quiz-launch').classList.remove('is-hidden');
  document.querySelector('#quiz-player').classList.add('is-hidden');
  document.querySelector('#quiz-player').replaceChildren();
  document.querySelector('#submission-result').classList.add('is-hidden');
}

function restoreDemoWorkspace() {
  try {
    return JSON.parse(window.localStorage.getItem(demoStorageKey())) || null;
  } catch {
    return null;
  }
}

function saveDemoWorkspace(value) {
  demoWorkspace = value;
  window.localStorage.setItem(demoStorageKey(), JSON.stringify(value));
}

function demoStorageKey() {
  return session?.user?.id ? `${DEMO_KEY}.${session.user.id}` : DEMO_KEY;
}

function selectedTeamStorageKey() {
  return session?.user?.id ? `sphinx.selected-team.${session.user.id}` : 'sphinx.selected-team';
}

function switchWorkspace(name) {
  if (name === 'studio' && selectedTeam && selectedTeam.role !== 'admin') {
    showToast('Admin Studio is available only to this team’s administrator.');
    name = 'play';
  }
  const panel = document.querySelector(`#${name}-workspace`);
  if (!panel) return;
  document.querySelectorAll('.workspace-tab').forEach((tab) => {
    tab.classList.toggle('is-active', tab.dataset.workspace === name);
  });
  document.querySelectorAll('.workspace-panel').forEach((panel) => panel.classList.add('is-hidden'));
  panel.classList.remove('is-hidden');
}

document.querySelectorAll('.workspace-tab').forEach((tab) => {
  tab.addEventListener('click', () => switchWorkspace(tab.dataset.workspace));
});

function renderPreparedQuestions() {
  const container = document.querySelector('#prepared-list');
  container.replaceChildren(...preparedQuestions.map((question, index) => {
    const card = document.createElement('article');
    card.className = 'prepared-question';
    const number = document.createElement('span');
    number.textContent = String(index + 1).padStart(2, '0');
    const copy = document.createElement('div');
    const title = document.createElement('strong');
    title.textContent = question.question_text;
    const detail = document.createElement('small');
    detail.textContent = `${question.options.length} options · answer: ${question.options[question.correct_option_index]}`;
    copy.append(title, detail);
    card.append(number, copy);
    return card;
  }));
  document.querySelector('#prepared-count').textContent = `${preparedQuestions.length} questions`;
}

document.querySelector('#add-custom-question').addEventListener('click', async (event) => {
  const question = document.querySelector('#custom-question').value.trim();
  const optionA = document.querySelector('#custom-option-a').value.trim();
  const optionB = document.querySelector('#custom-option-b').value.trim();
  if (!question || !optionA || !optionB) {
    showToast('Enter a question and both answer options.');
    return;
  }
  const draft = {
    question_text: question,
    options: [optionA, optionB],
    correct_option_index: Number(document.querySelector('#custom-correct').value)
  };
  const button = event.currentTarget;
  button.disabled = true;
  try {
    await apiRequest('/questions/validate', { method: 'POST', token: session.token, body: draft });
  } catch (error) {
    showToast(error.message);
    button.disabled = false;
    return;
  }
  preparedQuestions.push({
    ...draft,
    explanation: document.querySelector('#custom-explanation').value.trim() || 'Prepared by the quiz administrator.'
  });
  ['custom-question', 'custom-option-a', 'custom-option-b', 'custom-explanation']
    .forEach((id) => { document.querySelector(`#${id}`).value = ''; });
  renderPreparedQuestions();
  button.disabled = false;
  showToast('Question validated by Rust and added to the quiz.');
});

async function validatePreparedQuestions() {
  for (const question of preparedQuestions) {
    await apiRequest('/questions/validate', {
      method: 'POST',
      token: session.token,
      body: {
        question_text: question.question_text,
        options: question.options,
        correct_option_index: question.correct_option_index
      }
    });
  }
}

function setWorkflowStatus(message, kind = '') {
  const status = document.querySelector('#workflow-status');
  status.textContent = message;
  status.className = `workflow-status ${kind}`.trim();
}

function restoreDemoControls() {
  const quizInput = document.querySelector('#play-quiz-id');
  const resultsInput = document.querySelector('#results-quiz-id');
  quizInput.value = '';
  resultsInput.value = '';
  document.querySelector('#play-availability').textContent = 'Waiting for quiz';
  if (!demoWorkspace?.quizId) {
    updateStartAvailability();
    return;
  }
  quizInput.value = demoWorkspace.quizId;
  updateStartAvailability();
  document.querySelector('#play-availability').textContent = 'Published · ready';
  document.querySelector('#quiz-launch p').textContent = `${demoWorkspace.title} is published and ready to start.`;
  resultsInput.value = demoWorkspace.quizId;
  setWorkflowStatus(`Published “${demoWorkspace.title}”. Team ID: ${demoWorkspace.teamId}. Quiz ID: ${demoWorkspace.quizId}`, 'is-success');
}

function updateStartAvailability() {
  const quizId = document.querySelector('#play-quiz-id').value.trim();
  document.querySelector('#start-demo-quiz').disabled = !isUuid(quizId);
}

function isUuid(value) {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}

document.querySelector('#play-quiz-id').addEventListener('input', updateStartAvailability);

document.querySelector('#publish-demo').addEventListener('click', async (event) => {
  const button = event.currentTarget;
  const label = button.querySelector('span');
  const teamName = document.querySelector('#demo-team-name').value.trim();
  const title = document.querySelector('#demo-quiz-title').value.trim();
  const description = document.querySelector('#demo-quiz-description').value.trim();
  const maxAttempts = Number(document.querySelector('#demo-max-attempts').value);
  const durationMinutes = Number(document.querySelector('#demo-duration').value);
  const participantIds = selectedParticipantIds();
  const canUseSelectedTeam = selectedTeam?.role === 'admin';
  if ((!canUseSelectedTeam && !teamName) || !title) {
    showToast('Select an admin team or enter a new team name, then enter a quiz title.');
    return;
  }
  if (!participantIds.length) {
    showToast('Select at least one team member for this quiz.');
    return;
  }
  button.disabled = true;
  label.textContent = canUseSelectedTeam ? 'Using admin team…' : 'Creating team…';
  let team = selectedTeam;
  let quiz = null;
  try {
    label.textContent = 'Validating questions…';
    await validatePreparedQuestions();
    if (!canUseSelectedTeam) {
      team = await apiRequest('/api/v1/teams', {
        method: 'POST', token: session.token, body: { name: teamName }
      });
      await loadTeamContext(team.id);
    }
    label.textContent = 'Creating quiz…';
    quiz = await apiRequest(`/api/v1/teams/${team.id}/quizzes`, {
      method: 'POST', token: session.token,
      body: { title, description: description || null, max_attempts: maxAttempts }
    });
    label.textContent = `Assigning ${participantIds.length} members…`;
    await apiRequest(`/api/v1/quizzes/${quiz.id}/eligibilities`, {
      method: 'PUT', token: session.token, body: { user_ids: participantIds }
    });
    for (let index = 0; index < preparedQuestions.length; index += 1) {
      label.textContent = `Posting question ${index + 1}/${preparedQuestions.length}…`;
      await apiRequest(`/api/v1/teams/${team.id}/quizzes/${quiz.id}/questions`, {
        method: 'POST', token: session.token, body: preparedQuestions[index]
      });
    }
    label.textContent = 'Publishing…';
    await apiRequest(`/api/v1/quizzes/${quiz.id}/publish`, {
      method: 'POST', token: session.token, body: { duration_minutes: durationMinutes }
    });
    saveDemoWorkspace({ teamId: team.id, quizId: quiz.id, title });
    restoreDemoControls();
    await loadAvailableQuizzes(team.id);
    setWorkflowStatus(`Published “${title}” for ${participantIds.length} members. Quiz ID: ${quiz.id}`, 'is-success');
    showToast('Quiz published. Log in as a member to take it.');
  } catch (error) {
    const retained = quiz ? ` Draft quiz ${quiz.id} was retained for inspection.` : '';
    setWorkflowStatus(`${error.message}${retained}`, 'is-error');
    showToast(error.message);
  } finally {
    button.disabled = false;
    label.textContent = 'Create quiz, assign members & publish';
  }
});

function renderQuizPlayer(attempt) {
  const player = document.querySelector('#quiz-player');
  player.replaceChildren();
  const meta = document.createElement('p');
  meta.className = 'quiz-meta';
  meta.textContent = `Attempt ${attempt.attempt_number} of ${attempt.max_attempts} · closes ${formatDateTime(attempt.deadline, { timeStyle: 'short' })}`;
  player.append(meta);
  attempt.questions.forEach((question, questionIndex) => {
    const fieldset = document.createElement('fieldset');
    fieldset.className = 'play-question';
    const legend = document.createElement('legend');
    legend.textContent = `${questionIndex + 1}. ${question.question_text}`;
    fieldset.append(legend);
    question.options.forEach((option, optionIndex) => {
      const label = document.createElement('label');
      label.className = 'answer-option';
      const input = document.createElement('input');
      input.type = 'radio';
      input.name = `answer-${question.question_id}`;
      input.value = String(optionIndex);
      const text = document.createElement('span');
      text.textContent = option;
      label.append(input, text);
      fieldset.append(label);
    });
    player.append(fieldset);
  });
  const submit = document.createElement('button');
  submit.className = 'primary-button workspace-action';
  submit.type = 'submit';
  submit.innerHTML = '<span>Submit answers</span><b aria-hidden="true">→</b>';
  player.append(submit);
  player.classList.remove('is-hidden');
}

document.querySelector('#start-demo-quiz').addEventListener('click', async (event) => {
  const button = event.currentTarget;
  const quizId = document.querySelector('#play-quiz-id').value.trim();
  button.disabled = true;
  try {
    activeAttempt = await apiRequest(`/api/v1/quizzes/${quizId}/attempt-sessions`, {
      method: 'POST', token: session.token
    });
    document.querySelector('#quiz-launch').classList.add('is-hidden');
    document.querySelector('#submission-result').classList.add('is-hidden');
    renderQuizPlayer(activeAttempt);
  } catch (error) {
    showToast(error.message);
    button.disabled = false;
  }
});

document.querySelector('#quiz-player').addEventListener('submit', async (event) => {
  event.preventDefault();
  const player = event.currentTarget;
  const answers = activeAttempt.questions.map((question) => {
    const selected = player.querySelector(`input[name="answer-${question.question_id}"]:checked`);
    return selected ? { question_id: question.question_id, selected_option_index: Number(selected.value) } : null;
  });
  if (answers.some((answer) => answer === null)) {
    showToast('Answer every question before submitting.');
    return;
  }
  const button = player.querySelector('button[type="submit"]');
  button.disabled = true;
  try {
    const result = await apiRequest(`/api/v1/attempt-sessions/${activeAttempt.session_id}/submit`, {
      method: 'POST', token: session.token, body: { answers }
    });
    player.classList.add('is-hidden');
    const summary = document.querySelector('#submission-result');
    summary.textContent = `Submitted successfully: ${result.score}/${result.total_questions} (${formatPercentage(result.percentage)}). ${result.remaining_attempts} attempt(s) remaining.`;
    summary.classList.remove('is-hidden');
    document.querySelector('#results-quiz-id').value = result.quiz_id;
    showToast('Attempt saved in PostgreSQL.');
    setTimeout(() => {
      switchWorkspace('results');
      resultsForm.requestSubmit();
    }, 700);
  } catch (error) {
    showToast(error.message);
    button.disabled = false;
  }
});

function formatPercentage(value) {
  const number = Number(value);
  return value == null || !Number.isFinite(number) ? '—' : `${number.toFixed(1).replace(/\.0$/, '')}%`;
}

function formatDateTime(value, options) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? 'time unavailable' : new Intl.DateTimeFormat(undefined, options).format(date);
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
      formatDateTime(attempt.submitted_at, { dateStyle: 'medium', timeStyle: 'short' })
    ));
  renderList(document.querySelector('#attempt-results'), attempts, 'You have not submitted an attempt for this quiz.');
  resultsView.classList.remove('is-hidden');
}

resultsForm.addEventListener('submit', async (event) => {
  event.preventDefault();
  const quizInput = document.querySelector('#results-quiz-id');
  const quizId = quizInput.value.trim();
  const validUuid = isUuid(quizId);
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
    showToast(error.message);
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
