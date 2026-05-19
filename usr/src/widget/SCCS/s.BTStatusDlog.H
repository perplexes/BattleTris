h16007
s 00000/00000/00000
d R 1.2 01/10/20 13:35:17 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTStatusDlog.H
c Name history : 1 0 src/widget/BTStatusDlog.H
e
s 00025/00000/00000
d D 1.1 01/10/20 13:35:16 bmc 1 0
c date and time created 01/10/20 13:35:16 by bmc
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
/*    FILE: BTStatusDlog.H                                      */
/*    DATE: Tue Apr  5 02:55:13 1994                            */
/****************************************************************/

#ifndef BTSTATUSDLOG_H
#define BTSTATUSDLOG_H

#include "BTConfig.H"
#include "BTMessageDlog.H"

class BTStatusDlog : public BTMessageDlog {
public:
  BTStatusDlog(BTWidget *parent, char *message) : BTMessageDlog(parent, message) {
    XtUnmanageChild(XmMessageBoxGetChild(me_, XmDIALOG_OK_BUTTON));
    XtUnmanageChild(XmMessageBoxGetChild(me_, XmDIALOG_SEPARATOR));
    XtUnmanageChild(XmMessageBoxGetChild(me_, XmDIALOG_SYMBOL_LABEL));
  }

  ~BTStatusDlog() {}
};

#endif
E 1
