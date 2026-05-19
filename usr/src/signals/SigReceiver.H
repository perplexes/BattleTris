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
