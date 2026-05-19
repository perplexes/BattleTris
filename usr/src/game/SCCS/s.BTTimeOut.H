h13584
s 00000/00000/00000
d R 1.2 01/10/20 13:35:37 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTTimeOut.H
c Name history : 1 0 src/game/BTTimeOut.H
e
s 00030/00000/00000
d D 1.1 01/10/20 13:35:36 bmc 1 0
c date and time created 01/10/20 13:35:36 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTTimeOut.C                                         */
/*    ASSN:                                                     */
/*    DATE: Fri Apr 14 16:08:55 EDT 1995                        */
/****************************************************************/

#ifndef BTTIMEOUT_H
#define BTTIMEOUT_H

#include "BTConfig.H"
#include <X11/Intrinsic.h>

#define BT_TIMEOUT_CALLBACK(nm,func) \
  void func(unsigned long *ul); \
  static void func##_CB(void *ud, unsigned long *ul) \
  { ((nm*)ud)->func(ul); }


struct BTTimeOut {
  XtIntervalId rep_;
  int time_;
  int paused_;
  int active_;
  void (*callback_) (void *, unsigned long *);
  BTTimeOut() : rep_ (0), time_ (0), paused_ (0), active_ (0) {}
};

#endif 
E 1
