/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: SigHandler.H                                        */
/*    DATE: Sun Feb  6 14:26:14 1994                            */
/****************************************************************/

#ifndef SIGHANDLER_H
#define SIGHANDLER_H

#include "BTConfig.H"

enum SigDisposition {
  SIG_DEFAULT,
  SIG_IGNORED,
  SIG_BLOCKED,
  SIG_ENABLED
};

class SigHandler {

friend class SigReceiver;

private:
  SigDisposition disposition_;

public:
  inline SigHandler(SigDisposition disp = SIG_ENABLED) : disposition_(disp) {}
  inline virtual ~SigHandler() {}

  virtual void handle() = 0;

private:
  SigHandler(const SigHandler& other);
  SigHandler& operator=(const SigHandler& other);
};

#endif
