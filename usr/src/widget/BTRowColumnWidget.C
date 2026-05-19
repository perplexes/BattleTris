/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTRowColumnWidget.C                                 */
/*    ASGN: Final                                               */
/*    DATE: Thu Apr 21 14:18:44 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTRowColumnWidget.H"

BTRowColumnWidget::BTRowColumnWidget(BTWidget *parent, const char *name,
    Position x1, Position y1, Position x2, Position y2)
: BTWidget(parent)
{
	me_ = XtVaCreateWidget(name, xmRowColumnWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNtopAttachment, XmATTACH_POSITION,
	    XmNtopPosition, y1,
	    XmNleftAttachment, XmATTACH_POSITION,
	    XmNleftPosition, x1,
	    XmNrightAttachment, XmATTACH_POSITION,
	    XmNrightPosition,x2,
	    XmNbottomAttachment, XmATTACH_POSITION,
	    XmNbottomPosition, y2,
	    NULL);
}

BTRowColumnWidget::BTRowColumnWidget(BTWidget *parent, const char *name,
    Dimension width, Dimension height)
: BTWidget(parent)
{
	me_ = XtVaCreateWidget(name, xmRowColumnWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNwidth, width,
	    XmNheight, height,
	    NULL);
}

BTRowColumnWidget::BTRowColumnWidget(BTWidget *parent, const char *name)
: BTWidget(parent)
{
	me_ = XtVaCreateWidget(name, xmRowColumnWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNrowColumnType, XmWORK_AREA,
	    XmNpacking, XmPACK_COLUMN,
	    XmNorientation, XmVERTICAL,
	    XmNnumColumns, 1,
	    NULL);
}
