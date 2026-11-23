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
  document.querySelector('#signed-in-user-id').textContent = session.user.id;
  demoWorkspace = restoreDemoWorkspace();
  renderPreparedQuestions();
  restoreDemoControls();
  switchWorkspace('studio');
  loadTeamContext();
}

async function loadTeamContext(preferredTeamId = demoWorkspace?.teamId) {
  const studioTab = document.querySelector('[data-workspace="studio"]');
  const select = document.querySelector('#team-context-select');
  try {
    teamMemberships = await apiRequest('/api/v1/teams', { token: session.token });
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
    const storedTeamId = window.localStorage.getItem('sphinx.selected-team');
    select.value = teamMemberships.some((team) => team.id === preferredTeamId)
      ? preferredTeamId
      : teamMemberships.some((team) => team.id === storedTeamId) ? storedTeamId : teamMemberships[0].id;
    selectedTeam = teamMemberships.find((team) => team.id === select.value);
    applySelectedTeamRole();
    if (selectedTeam.role !== 'admin') {
      switchWorkspace('play');
      showToast('Member access: take eligible quizzes and view your results.');
    }
  } catch (error) {
    showToast(`Could not load team roles: ${error.message}`);
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
    window.localStorage.setItem('sphinx.selected-team', selectedTeam.id);
    loadAvailableQuizzes(selectedTeam.id);
  } else {
    renderAvailableQuizzes([]);
  }
}

document.querySelector('#team-context-select').addEventListener('change', (event) => {
  selectedTeam = teamMemberships.find((team) => team.id === event.target.value) || null;
  applySelectedTeamRole();
});

document.querySelector('#refresh-teams').addEventListener('click', () => loadTeamContext(selectedTeam?.id));

async function loadAvailableQuizzes(teamId) {
  const copy = document.querySelector('#available-copy');
  copy.textContent = 'Loading eligible quizzes…';
  try {
    const response = await apiRequest(`/api/v1/teams/${teamId}/quizzes/available`, { token: session.token });
    renderAvailableQuizzes(response.quizzes || []);
    copy.textContent = response.quizzes?.length
      ? 'Choose an open quiz from this team.'
      : 'No eligible open quizzes are available for this team.';
  } catch (error) {
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
    detail.textContent = `Open until ${new Intl.DateTimeFormat(undefined, { timeStyle: 'short' }).format(new Date(quiz.ends_at))}`;
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
  successState.classList.remove('is-visible');
  resultsView.classList.add('is-hidden');
  authWrap.classList.remove('results-mode');
  form.classList.remove('is-hidden');
  heading.classList.remove('is-hidden');
  form.reset();
  setMode('login');
  activeAttempt = null;
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

function switchWorkspace(name) {
  document.querySelectorAll('.workspace-tab').forEach((tab) => {
    tab.classList.toggle('is-active', tab.dataset.workspace === name);
  });
  document.querySelectorAll('.workspace-panel').forEach((panel) => panel.classList.add('is-hidden'));
  document.querySelector(`#${name}-workspace`).classList.remove('is-hidden');
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

document.querySelector('#add-custom-question').addEventListener('click', () => {
  const question = document.querySelector('#custom-question').value.trim();
  const optionA = document.querySelector('#custom-option-a').value.trim();
  const optionB = document.querySelector('#custom-option-b').value.trim();
  if (!question || !optionA || !optionB) {
    showToast('Enter a question and both answer options.');
    return;
  }
  preparedQuestions.push({
    question_text: question,
    options: [optionA, optionB],
    correct_option_index: Number(document.querySelector('#custom-correct').value),
    explanation: document.querySelector('#custom-explanation').value.trim() || 'Prepared by the quiz administrator.'
  });
  ['custom-question', 'custom-option-a', 'custom-option-b', 'custom-explanation']
    .forEach((id) => { document.querySelector(`#${id}`).value = ''; });
  renderPreparedQuestions();
  showToast('Question added to the prepared set.');
});

function setWorkflowStatus(message, kind = '') {
  const status = document.querySelector('#workflow-status');
  status.textContent = message;
  status.className = `workflow-status ${kind}`.trim();
}

function restoreDemoControls() {
  const start = document.querySelector('#start-demo-quiz');
  if (!demoWorkspace?.quizId) {
    updateStartAvailability();
    return;
  }
  document.querySelector('#play-quiz-id').value = demoWorkspace.quizId;
  updateStartAvailability();
  document.querySelector('#play-availability').textContent = 'Published · ready';
  document.querySelector('#quiz-launch p').textContent = `${demoWorkspace.title} is published and ready to start.`;
  document.querySelector('#results-quiz-id').value = demoWorkspace.quizId;
  setWorkflowStatus(`Published “${demoWorkspace.title}”. Team ID: ${demoWorkspace.teamId}. Quiz ID: ${demoWorkspace.quizId}`, 'is-success');
}

function updateStartAvailability() {
  const quizId = document.querySelector('#play-quiz-id').value.trim();
  document.querySelector('#start-demo-quiz').disabled = !/^[0-9a-f-]{36}$/i.test(quizId);
}

document.querySelector('#play-quiz-id').addEventListener('input', updateStartAvailability);

document.querySelector('#publish-demo').addEventListener('click', async (event) => {
  const button = event.currentTarget;
  const label = button.querySelector('span');
  const teamName = document.querySelector('#demo-team-name').value.trim();
  const title = document.querySelector('#demo-quiz-title').value.trim();
  const memberId = document.querySelector('#demo-member-id').value.trim();
  const canUseSelectedTeam = selectedTeam?.role === 'admin';
  if ((!canUseSelectedTeam && !teamName) || !title) {
    showToast('Select an admin team or enter a new team name, then enter a quiz title.');
    return;
  }
  button.disabled = true;
  label.textContent = canUseSelectedTeam ? 'Using admin team…' : 'Creating team…';
  try {
    let team = selectedTeam;
    if (!canUseSelectedTeam) {
      team = await apiRequest('/api/v1/teams', {
        method: 'POST', token: session.token, body: { name: teamName }
      });
      await loadTeamContext(team.id);
    }
    if (memberId) {
      label.textContent = 'Adding eligible member…';
      await apiRequest(`/api/v1/teams/${team.id}/members`, {
        method: 'POST', token: session.token, body: { user_id: memberId }
      });
    }
    label.textContent = 'Creating quiz…';
    const quiz = await apiRequest(`/api/v1/teams/${team.id}/quizzes`, {
      method: 'POST', token: session.token,
      body: { title, description: 'Prepared for the Sphinx presentation demo', max_attempts: 2 }
    });
    for (let index = 0; index < preparedQuestions.length; index += 1) {
      label.textContent = `Posting question ${index + 1}/${preparedQuestions.length}…`;
      await apiRequest(`/api/v1/teams/${team.id}/quizzes/${quiz.id}/questions`, {
        method: 'POST', token: session.token, body: preparedQuestions[index]
      });
    }
    label.textContent = 'Publishing…';
    await apiRequest(`/api/v1/quizzes/${quiz.id}/publish`, {
      method: 'POST', token: session.token, body: { duration_minutes: 15 }
    });
    saveDemoWorkspace({ teamId: team.id, quizId: quiz.id, title });
    restoreDemoControls();
    await loadAvailableQuizzes(team.id);
    if (memberId) {
      showToast('Quiz published. Share the quiz ID with the eligible member.');
    } else {
      switchWorkspace('play');
      showToast('Quiz published. The admin account can also start it as an eligible team member.');
    }
  } catch (error) {
    setWorkflowStatus(error.message, 'is-error');
    showToast(error.message);
  } finally {
    button.disabled = false;
    label.textContent = 'Create team, quiz & publish';
  }
});

function renderQuizPlayer(attempt) {
  const player = document.querySelector('#quiz-player');
  player.replaceChildren();
  const meta = document.createElement('p');
  meta.className = 'quiz-meta';
  meta.textContent = `Attempt ${attempt.attempt_number} of ${attempt.max_attempts} · closes ${new Intl.DateTimeFormat(undefined, { timeStyle: 'short' }).format(new Date(attempt.deadline))}`;
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
  const answers = activeAttempt.questions.map((question) => {
    const selected = event.currentTarget.querySelector(`input[name="answer-${question.question_id}"]:checked`);
    return selected ? { question_id: question.question_id, selected_option_index: Number(selected.value) } : null;
  });
  if (answers.some((answer) => answer === null)) {
    showToast('Answer every question before submitting.');
    return;
  }
  const button = event.currentTarget.querySelector('button[type="submit"]');
  button.disabled = true;
  try {
    const result = await apiRequest(`/api/v1/attempt-sessions/${activeAttempt.session_id}/submit`, {
      method: 'POST', token: session.token, body: { answers }
    });
    event.currentTarget.classList.add('is-hidden');
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
