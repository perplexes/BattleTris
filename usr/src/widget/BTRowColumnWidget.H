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
	BTRowColumnWidget(BTWidget *parent, const char *name);
	BTRowColumnWidget(BTWidget *parent, const char *name,
	    Dimension width, Dimension height);
	BTRowColumnWidget(BTWidget *parent, const char *name,
	    Position x1, Position y1, Position x2, Position y2);
	~BTRowColumnWidget() {}
};

#endif
