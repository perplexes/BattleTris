h28244
s 00000/00000/00000
d R 1.2 01/10/20 13:34:59 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/signals/SigReceiver.H
c Name history : 1 0 src/signals/SigReceiver.H
e
s 00077/00000/00000
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
/*    FILE: SigReceiver.H                                       */
/*    DATE: Sun Feb  6 02:38:06 1994                            */
/****************************************************************/

#ifndef SIGRECEIVER_H
#define SIGRECEIVER_H

#include "BTConfig.H"

#include <signal.h>

#include "SigHandler.H"

typedef RETSIGTYPE (*sigfunc_t)(int);

enum {
#ifdef NSIG
  SIGREC_MAX = NSIG	// Use #define constant if there is one
#else
  SIGREC_MAX = 50	// Good enough for most UNIX systems for a while
#endif
};

struct SigHdlInfo {
  SigDisposition disposition_;
  SigHandler *handler_;

  inline SigHdlInfo() : disposition_(SIG_DEFAULT), handler_(0) {}
};

class SigReceiver {

private:
  static SigHdlInfo handlers_[SIGREC_MAX];

private:
  static RETSIGTYPE receiver(int signum);
  void siginit(int signum, sigfunc_t handler);
  sigfunc_t sigfetch(int signum);

public:
  inline SigReceiver() {}
  inline ~SigReceiver() {}

  void reset();

  void install(int signum, SigHandler *handler);
  void remove(int signum);

  void enable(int signum);
  void disable(int signum);

  void block(int signum);
  void unblock(int signum);

  inline int defaulted(int signum) {
    return SigReceiver::sigfetch(signum) == (sigfunc_t) SIG_DFL;
  }

  inline int enabled(int signum) {
    return (handlers_[signum].handler_ != 0) &&
      (handlers_[signum].disposition_ == SIG_ENABLED);
  }

  inline int blocked(int signum) {
    return handlers_[signum].disposition_ == SIG_BLOCKED;
  }

private:
  SigReceiver(const SigReceiver& other);
  SigReceiver& operator=(const SigReceiver& other);
};

#endif
E 1
