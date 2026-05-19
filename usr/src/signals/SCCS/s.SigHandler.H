h17666
s 00000/00000/00000
d R 1.2 01/10/20 13:34:59 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/signals/SigHandler.H
c Name history : 1 0 src/signals/SigHandler.H
e
s 00038/00000/00000
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
E 1
