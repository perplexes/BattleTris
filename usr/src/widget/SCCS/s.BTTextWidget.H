h55976
s 00008/00009/00025
d D 1.2 01/10/21 19:25:19 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:17 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTTextWidget.H
c Name history : 1 0 src/widget/BTTextWidget.H
e
s 00034/00000/00000
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
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTTextWidget.H                                      */
/*    ASSN:                                                     */
/*    DATE: Thu Apr 28 23:12:10 1994                            */
/****************************************************************/

#ifndef BTTEXTWIDGET_H
#define BTTEXTWIDGET_H

#include "BTConfig.H"

#include <Xm/Text.h>

#include "BTWidget.H"

class BTTextWidget : public BTWidget {
D 3

E 3
protected:
D 3
  short rows_, cols_;
  short tab_;
E 3
I 3
	short rows_, cols_;
	short tab_;
E 3

public:
D 3
  BTTextWidget(BTWidget *parent, char *const name,
               Position x1, Position y1, Position x2, Position y2,
               short rows = 25, short cols = 80);
  ~BTTextWidget() {}
E 3
I 3
	BTTextWidget(BTWidget *parent, const char *name,
	    Position x1, Position y1, Position x2, Position y2,
	    short rows = 25, short cols = 80);
	~BTTextWidget() {}
E 3

D 3
  void setTab(short tab) { tab_ = tab; }
  void setText(short row, short tab_stop, char *text);
E 3
I 3
	void setTab(short tab) { tab_ = tab; }
	void setText(short row, short tab_stop, const char *text);
E 3
};

#endif
E 1
