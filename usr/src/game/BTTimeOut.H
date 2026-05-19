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
