h52897
s 00006/00007/00024
d D 1.2 01/10/21 19:25:14 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:13 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTFormWidget.H
c Name history : 1 0 src/widget/BTFormWidget.H
e
s 00031/00000/00000
d D 1.1 01/10/20 13:35:12 bmc 1 0
c date and time created 01/10/20 13:35:12 by bmc
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
/*    FILE: BTFormWidget.H                                      */
/*    ASGN: Images                                              */
/*    DATE: Wed Apr 20 17:59:36 1994                            */
/****************************************************************/

#ifndef BTFORMWIDGET_H
#define BTFORMWIDGET_H

#include "BTConfig.H"

#include <Xm/Form.h>

#include "BTWidget.H"

class BTFormWidget : public BTWidget {
D 3

E 3
protected:
D 3
  int frac_base_;
E 3
I 3
	int frac_base_;
E 3

public:
D 3
  BTFormWidget(BTWidget *parent, char *const name,
               Dimension width, Dimension height, int fraction_base);
  void placeChild( BTWidget *, Dimension, Dimension );
  void placeChild( BTWidget *, Dimension, Dimension, Dimension, Dimension );
  ~BTFormWidget() {}
E 3
I 3
	BTFormWidget(BTWidget *parent, const char *name,
	    Dimension width, Dimension height, int fraction_base);
	void placeChild(BTWidget *, Dimension, Dimension);
	void placeChild(BTWidget *, Dimension, Dimension, Dimension, Dimension);
	~BTFormWidget() {}
E 3
};

#endif
E 1
