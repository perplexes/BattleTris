h57283
s 00006/00006/00022
d D 1.2 01/10/21 19:25:17 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:16 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTRowColumnWidget.H
c Name history : 1 0 src/widget/BTRowColumnWidget.H
e
s 00028/00000/00000
d D 1.1 01/10/20 13:35:15 bmc 1 0
c date and time created 01/10/20 13:35:15 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTPushButtonWidget.H                                */
/*    ASGN: Images                                              */
/*    DATE: Tue Apr 19 20:04:17 1994                            */
/****************************************************************/

#ifndef BTROWCOLUMNWIDGET_H
#define BTROWCOLUMNWIDGET_H

#include "BTConfig.H"

#include <Xm/RowColumn.h>

#include "BTWidget.H"

class BTRowColumnWidget : public BTWidget {
public:
D 3
  BTRowColumnWidget(BTWidget *parent, char *const name);
  BTRowColumnWidget(BTWidget *parent, char *const name,
                    Dimension width, Dimension height);
  BTRowColumnWidget(BTWidget *parent, char *const name,
                    Position x1, Position y1, Position x2, Position y2);
  ~BTRowColumnWidget() {}
E 3
I 3
	BTRowColumnWidget(BTWidget *parent, const char *name);
	BTRowColumnWidget(BTWidget *parent, const char *name,
	    Dimension width, Dimension height);
	BTRowColumnWidget(BTWidget *parent, const char *name,
	    Position x1, Position y1, Position x2, Position y2);
	~BTRowColumnWidget() {}
E 3
};

#endif
E 1
