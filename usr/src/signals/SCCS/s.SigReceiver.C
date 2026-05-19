h22277
s 00000/00000/00000
d R 1.2 01/10/20 13:34:59 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/signals/SigReceiver.C
c Name history : 1 0 src/signals/SigReceiver.C
e
s 00204/00000/00000
d D 1.1 01/10/20 13:34:58 bmc 1 0
c date and time created 01/10/20 13:34:58 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: SigReceiver.C                                       */
/*    DATE: Sun Feb  6 03:04:14 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include "SigHandler.H"
#include "SigReceiver.H"

SigHdlInfo SigReceiver::handlers_[SIGREC_MAX];

void SigReceiver::reset()
{
#ifdef HAVE_SIGPROCMASK
  sigset_t set;

  sigfillset(&set);
  sigprocmask(SIG_UNBLOCK, &set, (sigset_t *) 0);
#else
  sigsetmask(0);
#endif

  for(int i = 1; i < SIGREC_MAX; i++) {
    handlers_[i].disposition_ = SIG_DEFAULT;
    SigReceiver::siginit(i, (sigfunc_t) SIG_DFL);
  }
}

void SigReceiver::install(int signum, SigHandler *handler)
{
  switch(handler->disposition_) {

  case SIG_DEFAULT:
    SigReceiver::siginit(signum, (sigfunc_t) SIG_DFL);
    break;

  case SIG_IGNORED:
    SigReceiver::siginit(signum, (sigfunc_t) SIG_IGN);
    break;

  case SIG_BLOCKED:
    SigReceiver::siginit(signum, SigReceiver::receiver);
    SigReceiver::block(signum);
    break;

  default:
    SigReceiver::siginit(signum, SigReceiver::receiver);
  }

  handlers_[signum].disposition_ = handler->disposition_;
  handlers_[signum].handler_ = handler;
}

void SigReceiver::remove(int signum)
{
  SigReceiver::siginit(signum, (sigfunc_t) SIG_DFL);

  if(handlers_[signum].disposition_ != SIG_BLOCKED)
    handlers_[signum].disposition_ = SIG_DEFAULT;
  handlers_[signum].handler_ = 0;
}

void SigReceiver::enable(int signum)
{
  if(handlers_[signum].handler_ != 0) {
    SigReceiver::siginit(signum, SigReceiver::receiver);
    if(handlers_[signum].disposition_ != SIG_BLOCKED)
      handlers_[signum].disposition_ = SIG_ENABLED;
  } else {
    SigReceiver::siginit(signum, (sigfunc_t) SIG_DFL);
    if(handlers_[signum].disposition_ != SIG_BLOCKED)
      handlers_[signum].disposition_ = SIG_DEFAULT;
  }
}

void SigReceiver::disable(int signum)
{
  SigReceiver::siginit(signum, (sigfunc_t) SIG_IGN);

  if(handlers_[signum].disposition_ != SIG_BLOCKED)
    handlers_[signum].disposition_ = SIG_IGNORED;
}

void SigReceiver::block(int signum)
{
#ifdef HAVE_SIGPROCMASK
  sigset_t set;

  sigemptyset(&set);
  sigaddset(&set, signum);
  sigprocmask(SIG_BLOCK, &set, (sigset_t *) 0);
#else
  int mask = sigmask(signum);

  sigblock(mask);
#endif

  handlers_[signum].disposition_ = SIG_BLOCKED;
}

void SigReceiver::unblock(int signum)
{
#ifdef HAVE_SIGPROCMASK
  sigset_t set;

  sigemptyset(&set);
  sigaddset(&set, signum);
  sigprocmask(SIG_UNBLOCK, &set, (sigset_t *) 0);
#else
  int oldmask = sigsetmask(0);
  int newmask = sigmask(signum);

  sigsetmask(oldmask & (~newmask));
#endif

  sigfunc_t handler = SigReceiver::sigfetch(signum);

  if(handler == (sigfunc_t) SIG_DFL)
    handlers_[signum].disposition_ = SIG_DEFAULT;
  else if(handler == (sigfunc_t) SIG_IGN)
    handlers_[signum].disposition_ = SIG_IGNORED;
  else
    handlers_[signum].disposition_ = SIG_ENABLED;
}

RETSIGTYPE SigReceiver::receiver(int signum)
{
  if(SigReceiver::handlers_[signum].handler_ != 0) {
    if(SigReceiver::handlers_[signum].disposition_ == SIG_ENABLED)
      (SigReceiver::handlers_[signum].handler_)->handle();
  }

#ifdef RETSIGVAL
  return RETSIGVAL;
#endif
}

void SigReceiver::siginit(int signum, sigfunc_t handler)
{
#ifdef HAVE_SIGACTION
  struct sigaction act;

  sigemptyset(&act.sa_mask);

  // Skanky cast because stupid C headers give wrong prototype for sa_handler
  // This just avoid the ANSI C++ warning about casting (*)(int) to (*)()

  *((sigfunc_t *) &act.sa_handler) = handler;

  // Skanky cast is done now ... resume normal nice programming

#ifdef SA_SIGINFO
  act.sa_flags = SA_SIGINFO;		// Use SA_SIGINFO if they\'ve got it
#else
  act.sa_flags = 0;			// No flags otherwise
#endif

  if(signum == SIGALRM) {
#ifdef SA_INTERRUPT
    act.sa_flags |= SA_INTERRUPT;	// SunOS 4.1.x
#endif
  } else {
#ifdef SA_RESTART
    act.sa_flags |= SA_RESTART;		// SVR4, 4.4BSD
#endif
  }

  sigaction(signum, &act, (struct sigaction *) 0);
#else
  struct sigvec act;

  // Skanky cast because stupid C headers give wrong prototype for sa_handler
  // This just avoid the ANSI C++ warning about casting (*)(int) to (*)()

  *((sigfunc_t *) &act.sv_handler) = handler;

  act.sv_mask = 0;
  act.sv_flags = 0;

  if(signum == SIGALRM) {
#ifdef SV_INTERRUPT
    act.sv_flags |= SV_INTERRUPT;
#endif
  }

  sigvector(signum, &act, (struct sigvec *) 0);
#endif
}

sigfunc_t SigReceiver::sigfetch(int signum)
{
#ifdef HAVE_SIGACTION
  struct sigaction act;
  sigaction(signum, (struct sigaction *) 0, &act);
  return (sigfunc_t) act.sa_handler;
#else
  struct sigvec act;
  sigvector(signum, (struct sigvec *) 0, &act);
  return (sigfunc_t) act.sv_handler;
#endif
}
E 1
