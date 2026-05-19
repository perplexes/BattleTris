h57288
s 00000/00000/00000
d R 1.2 01/10/20 13:35:33 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTSigHandlers.H
c Name history : 1 0 src/game/BTSigHandlers.H
e
s 00055/00000/00000
d D 1.1 01/10/20 13:35:32 bmc 1 0
c date and time created 01/10/20 13:35:32 by bmc
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
/*    FILE: BTSigHandlers.H                                     */
/*    DATE: Tue May  3 01:59:10 1994                            */
/****************************************************************/

#ifndef BTSIGHANDLERS_H
#define BTSIGHANDLERS_H

#include "BTConfig.H"

#include <iostream.h>

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#include "SigHandler.H"
#include "BTStartup.H"
#include "BattleTris.H"

static Boolean termsig_received = False;

class BTSigTermHandler : public SigHandler {
public:
  BTSigTermHandler(SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp) {}
  virtual ~BTSigTermHandler() {}

  void handle() {
    // If we\'ve already been called, don't write anything to the
    // pipe -- we might block on the write since we own both ends of it

    if(termsig_received == False) {
      int packet = 1;
      write(g_signalfds[1], (char *) &packet, sizeof(packet));
      termsig_received = True;
    }
  }
};

class BTSigPipeHandler : public SigHandler {
public:
  BTSigPipeHandler(SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp) {}
  virtual ~BTSigPipeHandler() {}

  void handle() {
    if(g_startup)
      g_startup->childDied();
  }
};

#endif
E 1
