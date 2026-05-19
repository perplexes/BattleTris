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
	BTFrameWidget(BTWidget *parent, const char *name,
	    Position x1, Position y1, Position x2, Position y2);
	BTFrameWidget(BTWidget *parent, const char *name);
	~BTFrameWidget() {}
};

#endif
