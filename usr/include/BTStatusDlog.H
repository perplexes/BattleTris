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
