h31715
s 00004/00004/00021
d D 1.2 01/10/21 19:25:15 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:18 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTFrameWidget.H
c Name history : 1 0 src/widget/BTFrameWidget.H
e
s 00025/00000/00000
d D 1.1 01/10/20 13:35:17 bmc 1 0
c date and time created 01/10/20 13:35:17 by bmc
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
/*    FILE: BTFrameWidget.H                                     */
/*    DATE: Tue Apr  5 02:55:13 1994                            */
/****************************************************************/

#ifndef BTFRAMEWIDGET_H
#define BTFRAMEWIDGET_H

#include "BTConfig.H"

#include <Xm/Frame.h>

#include "BTWidget.H"

class BTFrameWidget : public BTWidget {
public:
D 3
  BTFrameWidget(BTWidget *parent, char *const name,
                Position x1, Position y1, Position x2, Position y2);
  BTFrameWidget(BTWidget *parent, char *const name);
  ~BTFrameWidget() {}
E 3
I 3
	BTFrameWidget(BTWidget *parent, const char *name,
	    Position x1, Position y1, Position x2, Position y2);
	BTFrameWidget(BTWidget *parent, const char *name);
	~BTFrameWidget() {}
E 3
};

#endif
E 1
