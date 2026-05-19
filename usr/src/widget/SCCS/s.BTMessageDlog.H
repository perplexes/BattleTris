h31432
s 00004/00005/00025
d D 1.2 01/10/21 19:25:16 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:15 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTMessageDlog.H
c Name history : 1 0 src/widget/BTMessageDlog.H
e
s 00030/00000/00000
d D 1.1 01/10/20 13:35:14 bmc 1 0
c date and time created 01/10/20 13:35:14 by bmc
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
/*    FILE: BTMessageDlog.H                                     */
/*    DATE: Tue Apr  5 02:55:13 1994                            */
/****************************************************************/

#ifndef BTMESSAGEDLOG_H
#define BTMESSAGEDLOG_H

#include "BTConfig.H"

#include <Xm/MessageB.h>

class BTWidget;

class BTMessageDlog {
D 3

E 3
protected:
D 3
  Widget me_;
E 3
I 3
	Widget me_;
E 3

private:
D 3
  static void accept_cb(Widget widget, XtPointer data, XtPointer cbs);
E 3
I 3
	static void accept_cb(Widget widget, XtPointer data, XtPointer cbs);
E 3

public:
D 3
  BTMessageDlog(BTWidget *parent, char *message);
  virtual ~BTMessageDlog() {}
E 3
I 3
	BTMessageDlog(BTWidget *parent, const char *message);
	virtual ~BTMessageDlog() {}
E 3
};

#endif
E 1
