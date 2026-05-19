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
protected:
	int frac_base_;

public:
	BTFormWidget(BTWidget *parent, const char *name,
	    Dimension width, Dimension height, int fraction_base);
	void placeChild(BTWidget *, Dimension, Dimension);
	void placeChild(BTWidget *, Dimension, Dimension, Dimension, Dimension);
	~BTFormWidget() {}
};

#endif
