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
protected:
	Widget me_;

private:
	static void accept_cb(Widget widget, XtPointer data, XtPointer cbs);

public:
	BTMessageDlog(BTWidget *parent, const char *message);
	virtual ~BTMessageDlog() {}
};

#endif
