const form = document.querySelector('#auth-form');
const authWrap = document.querySelector('.auth-wrap');
const heading = document.querySelector('.auth-heading');
const nameFields = document.querySelector('#name-fields');
const termsRow = document.querySelector('#terms-row');
const password = document.querySelector('#password');
const primaryButton = document.querySelector('.primary-button span');
const successState = document.querySelector('#success-state');
const toast = document.querySelector('#toast');

let mode = 'login';

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

form.addEventListener('submit', (event) => {
  event.preventDefault();
  if (!validate()) return;
  form.classList.add('is-hidden');
  heading.classList.add('is-hidden');
  successState.classList.add('is-visible');
  successState.querySelector('h2').textContent = mode === 'signup' ? 'Welcome to Sphinx.' : 'Nice to see you.';
});

document.querySelectorAll('input').forEach((input) => {
  input.addEventListener('input', () => {
    if (input.closest('.field')) setError(input, '');
  });
});

document.querySelector('#reset-demo').addEventListener('click', () => {
  successState.classList.remove('is-visible');
  form.classList.remove('is-hidden');
  heading.classList.remove('is-hidden');
  form.reset();
  setMode('login');
});

document.querySelectorAll('[data-provider]').forEach((button) => {
  button.addEventListener('click', () => showToast(`${button.dataset.provider} sign-in is ready to connect.`));
});

document.querySelector('.forgot-link').addEventListener('click', () => showToast('Password recovery is ready to connect.'));

let toastTimer;
function showToast(message) {
  toast.textContent = message;
  toast.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove('show'), 2800);
}
